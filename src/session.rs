use std::fs;
use std::io;
use std::path::PathBuf;

/// Everything needed to fully reconstruct one tab across a restart:
/// enough to re-derive its live state without a network round-trip, the
/// same way waking a compressed tab within one run does.
pub struct SessionTab {
    pub url: String,
    pub title: String,
    pub history: Vec<String>,
    pub history_pos: usize,
    /// Whether this was the tab on screen when the session was saved.
    pub active: bool,
    /// LZ4-compressed source HTML (`lz4_flex::compress_prepend_size`) --
    /// the same representation `Tab::compressed_html` uses, regardless of
    /// which in-memory tier the tab was actually in when saved.
    pub compressed_html: Vec<u8>,
}

const MAGIC: &[u8; 4] = b"MLS1";

/// Reads and writes the on-disk session file: the list of open tabs, so a
/// restart can offer to restore them instead of starting from a blank
/// slate. Distinct from `swap::SwapFile`, which is an ephemeral, deleted-
/// on-exit RAM extension -- this file is meant to survive a restart, so
/// it lives in a real per-user data directory, not a temp file.
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new() -> Self {
        Self { path: crate::paths::data_dir().join("session.bin") }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// Writes `tabs` to disk, replacing whatever was there before. Errors
    /// (disk full, permissions) are for the caller to decide how to
    /// handle -- typically ignored, since failing to persist a session
    /// shouldn't interrupt browsing.
    pub fn save(&self, tabs: &[SessionTab]) -> io::Result<()> {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        write_u32(&mut buf, tabs.len() as u32);
        for tab in tabs {
            write_string(&mut buf, &tab.url);
            write_string(&mut buf, &tab.title);
            write_u32(&mut buf, tab.history.len() as u32);
            for entry in &tab.history {
                write_string(&mut buf, entry);
            }
            write_u32(&mut buf, tab.history_pos as u32);
            buf.push(tab.active as u8);
            write_u32(&mut buf, tab.compressed_html.len() as u32);
            buf.extend_from_slice(&tab.compressed_html);
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Write to a temp file and rename over the real one, so a process
        // killed mid-save leaves either the old session file or the new
        // one intact, never a truncated/corrupt one in its place.
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, &buf)?;
        fs::rename(&tmp, &self.path)
    }

    /// The saved tabs, or an empty list if there's no session file (the
    /// common case: a normal first run, or one that already restored/
    /// discarded and hasn't saved since). A corrupt file is treated as an
    /// `InvalidData` error rather than silently as "no session", so the
    /// caller can decide whether to log it.
    pub fn load(&self) -> io::Result<Vec<SessionTab>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        parse(&bytes).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "corrupt melora session file"))
    }

    /// Deletes the session file, if any. Used once its tabs have been
    /// either restored (a fresh save will follow as browsing continues)
    /// or explicitly discarded by the user.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u32(&mut self) -> Option<u32> {
        let bytes = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let b = self.data.get(self.pos..self.pos + len)?;
        self.pos += len;
        Some(b)
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.bytes(len)?.to_vec()).ok()
    }
}

fn parse(bytes: &[u8]) -> Option<Vec<SessionTab>> {
    if bytes.len() < 4 || &bytes[0..4] != MAGIC {
        return None;
    }
    let mut cur = Cursor { data: bytes, pos: 4 };
    let count = cur.u32()? as usize;
    let mut tabs = Vec::with_capacity(count);
    for _ in 0..count {
        let url = cur.string()?;
        let title = cur.string()?;
        let history_len = cur.u32()? as usize;
        let mut history = Vec::with_capacity(history_len);
        for _ in 0..history_len {
            history.push(cur.string()?);
        }
        let history_pos = cur.u32()? as usize;
        let active = *cur.bytes(1)?.first()? != 0;
        let html_len = cur.u32()? as usize;
        let compressed_html = cur.bytes(html_len)?.to_vec();
        tabs.push(SessionTab { url, title, history, history_pos, active, compressed_html });
    }
    Some(tabs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> SessionStore {
        let path = std::env::temp_dir().join(format!(
            "melora-session-test-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        SessionStore::at(path)
    }

    fn sample_tabs() -> Vec<SessionTab> {
        vec![
            SessionTab {
                url: "https://example.test/a".to_string(),
                title: "A".to_string(),
                history: vec!["https://example.test/a".to_string()],
                history_pos: 0,
                active: false,
                compressed_html: lz4_flex::compress_prepend_size(b"<html>a</html>"),
            },
            SessionTab {
                url: "https://example.test/c".to_string(),
                title: "C".to_string(),
                history: vec![
                    "https://example.test/b".to_string(),
                    "https://example.test/c".to_string(),
                ],
                history_pos: 1,
                active: true,
                compressed_html: lz4_flex::compress_prepend_size(b"<html>c</html>"),
            },
        ]
    }

    #[test]
    fn loading_with_no_file_present_returns_an_empty_list() {
        let store = temp_store();
        assert_eq!(store.load().unwrap().len(), 0);
    }

    #[test]
    fn round_trips_saved_tabs_exactly() {
        let store = temp_store();
        let original = sample_tabs();
        store.save(&original).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].url, "https://example.test/a");
        assert_eq!(loaded[0].title, "A");
        assert_eq!(loaded[0].history, vec!["https://example.test/a".to_string()]);
        assert_eq!(loaded[0].history_pos, 0);
        assert!(!loaded[0].active);
        assert_eq!(loaded[0].compressed_html, original[0].compressed_html);

        assert_eq!(loaded[1].url, "https://example.test/c");
        assert_eq!(loaded[1].history.len(), 2);
        assert_eq!(loaded[1].history_pos, 1);
        assert!(loaded[1].active);

        let _ = store.clear();
    }

    #[test]
    fn saving_again_replaces_the_previous_session() {
        let store = temp_store();
        store.save(&sample_tabs()).unwrap();
        store.save(&[]).unwrap();
        assert_eq!(store.load().unwrap().len(), 0);
        let _ = store.clear();
    }

    #[test]
    fn clear_removes_the_file_and_is_a_no_op_if_already_gone() {
        let store = temp_store();
        store.save(&sample_tabs()).unwrap();
        store.clear().unwrap();
        assert_eq!(store.load().unwrap().len(), 0);
        // Clearing again (no file present) must not error.
        store.clear().unwrap();
    }

    #[test]
    fn a_corrupt_file_is_reported_as_an_error_not_silently_treated_as_empty() {
        let store = temp_store();
        fs::write(&store.path, b"not a melora session file").unwrap();
        assert!(store.load().is_err());
        let _ = store.clear();
    }
}
