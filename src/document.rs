use std::fs;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(windows)]
    volume: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl FileIdentity {
    #[cfg(unix)]
    fn from_metadata(meta: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            dev: meta.dev(),
            ino: meta.ino(),
        }
    }

    #[cfg(windows)]
    fn from_metadata(meta: &fs::Metadata) -> Self {
        use std::os::windows::fs::MetadataExt;
        FileIdentity {
            volume: meta.volume_serial_number().unwrap_or(0),
            file_index: meta.file_index().unwrap_or(0),
        }
    }
}

/// Whether the on-disk file grew in place, or was rotated/truncated and
/// needs a full reload.
pub enum RefreshOutcome {
    Unchanged,
    Appended,
    NeedsReload,
}

/// The full contents of the file we're viewing, held in memory, plus a
/// complete index of logical line start offsets. Files are assumed to
/// always fit comfortably in memory, so no lazy/partial indexing is needed.
pub struct Document {
    buf: Vec<u8>,
    line_starts: Vec<u64>,
    file_id: FileIdentity,
}

impl Document {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let meta = fs::metadata(path)?;
        anyhow::ensure!(
            meta.is_file(),
            "wless can only view regular files, not {:?}",
            path
        );
        let buf = fs::read(path)?;
        let file_id = FileIdentity::from_metadata(&meta);
        let mut doc = Document {
            buf,
            line_starts: Vec::new(),
            file_id,
        };
        doc.rebuild_index();
        Ok(doc)
    }

    fn rebuild_index(&mut self) {
        self.line_starts.clear();
        self.line_starts.push(0);
        for pos in memchr::memchr_iter(b'\n', &self.buf) {
            let next = pos as u64 + 1;
            if next < self.buf.len() as u64 {
                self.line_starts.push(next);
            }
        }
    }

    /// Re-read the file if it has grown on disk since we last read it,
    /// appending only the new tail bytes and extending the index. Returns
    /// `NeedsReload` if the caller should call `reload` instead (the file
    /// shrank or its identity changed, indicating truncation/rotation).
    pub fn refresh_append(&mut self, path: &Path) -> anyhow::Result<RefreshOutcome> {
        let meta = fs::metadata(path)?;
        let new_id = FileIdentity::from_metadata(&meta);
        let new_len = meta.len();
        let old_len = self.buf.len() as u64;

        if new_id != self.file_id || new_len < old_len {
            return Ok(RefreshOutcome::NeedsReload);
        }
        if new_len == old_len {
            return Ok(RefreshOutcome::Unchanged);
        }

        use std::io::{Read, Seek, SeekFrom};
        let mut f = fs::File::open(path)?;
        f.seek(SeekFrom::Start(old_len))?;
        let mut tail = Vec::with_capacity((new_len - old_len) as usize);
        f.read_to_end(&mut tail)?;

        // If the file previously ended with a complete line (its last byte
        // was '\n'), the appended bytes begin an entirely new logical line
        // that needs its own line_starts entry -- otherwise we're just
        // continuing whatever unterminated line was already the last entry
        // (whose start offset is already recorded, so no push needed here).
        let prev_ended_with_newline = old_len > 0 && self.buf.last() == Some(&b'\n');
        if prev_ended_with_newline {
            self.line_starts.push(old_len);
        }

        self.buf.extend_from_slice(&tail);
        for pos in memchr::memchr_iter(b'\n', &tail) {
            let next = old_len + pos as u64 + 1;
            if next < self.buf.len() as u64 {
                self.line_starts.push(next);
            }
        }

        Ok(RefreshOutcome::Appended)
    }

    pub fn reload(&mut self, path: &Path) -> anyhow::Result<()> {
        *self = Document::open(path)?;
        Ok(())
    }

    pub fn len(&self) -> u64 {
        self.buf.len() as u64
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line_span(&self, idx: usize) -> Range<u64> {
        let start = self.line_starts[idx];
        let end = self
            .line_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.buf.len() as u64);
        start..end
    }

    /// Bytes of a logical line, with any trailing `\n` (and `\r`) trimmed.
    pub fn line_bytes(&self, idx: usize) -> &[u8] {
        let span = self.line_span(idx);
        let mut bytes = &self.buf[span.start as usize..span.end as usize];
        if bytes.last() == Some(&b'\n') {
            bytes = &bytes[..bytes.len() - 1];
        }
        if bytes.last() == Some(&b'\r') {
            bytes = &bytes[..bytes.len() - 1];
        }
        bytes
    }

    pub fn line_text(&self, idx: usize) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.line_bytes(idx))
    }

    pub fn line_offset(&self, idx: usize) -> u64 {
        self.line_starts[idx]
    }

    pub fn last_line_index(&self) -> usize {
        self.line_count().saturating_sub(1)
    }

    /// Round a byte offset within a line down to the nearest UTF-8 char
    /// boundary. Regex matches on `line_bytes` (raw bytes) aren't
    /// guaranteed to land on char boundaries of the lossily-decoded
    /// `line_text` -- e.g. a zero-width match from an empty/optional
    /// pattern can fall mid-character -- so callers that use a match
    /// offset to index into `line_text` must snap it first.
    pub fn floor_char_boundary(&self, line_idx: usize, offset: usize) -> usize {
        let text = self.line_text(line_idx);
        let mut offset = offset.min(text.len());
        while offset > 0 && !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}
