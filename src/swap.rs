use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Where a tab's compressed bytes live once written to a [`SwapFile`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapSlot {
    pub offset: u64,
    pub len: u32,
}

/// A small on-disk overflow tier underneath the RAM-compressed tab tier
/// (see `TabState::Swapped` in `tabs.rs`): once too many compressed tabs
/// pile up in RAM, the coldest ones get written here instead of staying
/// resident, trading a little disk I/O on wake for not holding their
/// compressed bytes in memory at all -- the browser's own small swapfile,
/// the same idea as OS-level swap applied to one process's own cold data.
///
/// Deliberately simple: append-only, one file per process, deleted on
/// drop (this is a RAM-extension for the current run, not persistent
/// storage -- see `session.rs` for the separate, actually-persistent
/// session file). Space from tabs that get woken or closed is never
/// reclaimed within a run; a real allocator would track a free list, but
/// tab churn within one run is bounded, and this exists to shave RAM, not
/// to be a general-purpose disk allocator.
pub struct SwapFile {
    file: File,
    path: PathBuf,
    next_offset: u64,
}

impl SwapFile {
    pub fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "melora-swap-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path)?;
        Ok(Self { file, path, next_offset: 0 })
    }

    /// Appends `bytes` to the file and returns the slot to read them back
    /// with.
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<SwapSlot> {
        self.file.seek(SeekFrom::Start(self.next_offset))?;
        self.file.write_all(bytes)?;
        let slot = SwapSlot { offset: self.next_offset, len: bytes.len() as u32 };
        self.next_offset += bytes.len() as u64;
        Ok(slot)
    }

    pub fn read(&mut self, slot: SwapSlot) -> std::io::Result<Vec<u8>> {
        self.file.seek(SeekFrom::Start(slot.offset))?;
        let mut buf = vec![0u8; slot.len as usize];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl Drop for SwapFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_back_multiple_slots_at_their_own_offsets() {
        let mut swap = SwapFile::new().unwrap();
        let a = swap.write(b"hello").unwrap();
        let b = swap.write(b"a much longer second entry, to make sure offsets don't overlap").unwrap();
        let c = swap.write(b"").unwrap();

        assert_eq!(swap.read(a).unwrap(), b"hello");
        assert_eq!(
            swap.read(b).unwrap(),
            b"a much longer second entry, to make sure offsets don't overlap"
        );
        assert_eq!(swap.read(c).unwrap(), b"");
        // Re-reading doesn't consume/move anything -- same slot, same bytes.
        assert_eq!(swap.read(a).unwrap(), b"hello");
    }

    #[test]
    fn the_backing_file_is_removed_when_the_swapfile_is_dropped() {
        let swap = SwapFile::new().unwrap();
        let path = swap.path.clone();
        assert!(path.exists());
        drop(swap);
        assert!(!path.exists());
    }
}
