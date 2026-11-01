use crate::cli::CountConfig;
use crate::error::LineCountError;
use crate::stats::FileStats;
use memchr::{memchr, memchr_iter};
use memmap2::MmapOptions;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::Path;
use std::time::Instant;

#[derive(Default)]
struct DetailedScanState {
    lines_newline: u64,
    crlf_count: u64,
    empty_lines: u64,
    words_count: u64,
    bytes_count: u64,
    is_binary: bool,
    has_utf8_bom: bool,
    has_invalid_utf8: bool,
    checked_bom: bool,
    prev_was_cr: bool,
    line_has_non_ws: bool,
    in_word: bool,
    last_byte: Option<u8>,
}

impl DetailedScanState {
    #[inline]
    fn process_chunk(&mut self, chunk: &[u8], strict_utf8: bool) {
        if chunk.is_empty() {
            return;
        }
        if !self.is_binary && memchr(0x00, chunk).is_some() {
            self.is_binary = true;
        }
        if strict_utf8 && !self.has_invalid_utf8 && std::str::from_utf8(chunk).is_err() {
            self.has_invalid_utf8 = true;
        }

        let mut scan_slice = chunk;
        if !self.checked_bom {
            self.checked_bom = true;
            if chunk.starts_with(&[0xEF, 0xBB, 0xBF]) {
                self.has_utf8_bom = true;
                scan_slice = &chunk[3..];
            } else if chunk.starts_with(&[0xFF, 0xFE]) || chunk.starts_with(&[0xFE, 0xFF]) {
                self.is_binary = true;
                self.has_invalid_utf8 = true;
            }
        }

        self.bytes_count += chunk.len() as u64;
        self.last_byte = Some(chunk[chunk.len() - 1]);

        for &b in scan_slice {
            if b == b'\n' {
                self.lines_newline += 1;
                if self.prev_was_cr {
                    self.crlf_count += 1;
                }
                if !self.line_has_non_ws {
                    self.empty_lines += 1;
                }
                self.line_has_non_ws = false;
                self.in_word = false;
                self.prev_was_cr = false;
            } else {
                self.prev_was_cr = b == b'\r';
                let is_ws = matches!(b, b' ' | b'\t' | b'\r' | 0x0B | 0x0C);
                if !is_ws {
                    self.line_has_non_ws = true;
                    if !self.in_word {
                        self.in_word = true;
                        self.words_count += 1;
                    }
                } else {
                    self.in_word = false;
                }
            }
        }
    }

    fn finalize(
        self,
        source_name: String,
        logical_mode: bool,
        strict_utf8: bool,
        elapsed_us: u64,
    ) -> Result<FileStats, LineCountError> {
        if strict_utf8 && (self.is_binary || self.has_invalid_utf8) {
            return Err(LineCountError::InvalidUtf8OrBinary {
                path: source_name,
                reason: String::from(
                    "виявлено нульові байти, маркер UTF-16 або невалідну послідовність UTF-8",
                ),
            });
        }

        let effective_text_bytes = if self.has_utf8_bom {
            self.bytes_count.saturating_sub(3)
        } else {
            self.bytes_count
        };
        let has_unterminated = effective_text_bytes > 0 && self.last_byte != Some(b'\n');
        let lines_logical = if effective_text_bytes == 0 {
            0
        } else if has_unterminated {
            self.lines_newline + 1
        } else {
            self.lines_newline
        };

        let mut empty_lines = self.empty_lines;
        if logical_mode && has_unterminated && !self.line_has_non_ws {
            empty_lines += 1;
        }

        let lf_only_count = self.lines_newline.saturating_sub(self.crlf_count);

        Ok(FileStats {
            source_name,
            lines_newline: self.lines_newline,
            lines_logical,
            crlf_count: self.crlf_count,
            lf_only_count,
            empty_lines,
            words_count: self.words_count,
            bytes_count: self.bytes_count,
            is_binary: self.is_binary,
            has_utf8_bom: self.has_utf8_bom,
            elapsed_us,
        })
    }
}

pub fn count_reader_fast<R: Read>(
    reader: R,
    source_name: String,
    buffer_size_bytes: usize,
) -> Result<FileStats, LineCountError> {
    let start = Instant::now();
    let mut buf_reader = BufReader::with_capacity(buffer_size_bytes, reader);
    let mut lines_newline: u64 = 0;
    let mut bytes_count: u64 = 0;
    let mut last_byte: Option<u8> = None;

    loop {
        let buffer = match buf_reader.fill_buf() {
            Ok(buf) => buf,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(LineCountError::from_io(source_name, e)),
        };

        let len = buffer.len();
        if len == 0 {
            break;
        }

        lines_newline += memchr_iter(b'\n', buffer).count() as u64;
        bytes_count += len as u64;
        last_byte = Some(buffer[len - 1]);

        buf_reader.consume(len);
    }

    let has_unterminated = bytes_count > 0 && last_byte != Some(b'\n');
    let lines_logical = if bytes_count == 0 {
        0
    } else if has_unterminated {
        lines_newline + 1
    } else {
        lines_newline
    };

    Ok(FileStats {
        source_name,
        lines_newline,
        lines_logical,
        crlf_count: 0,
        lf_only_count: lines_newline,
        empty_lines: 0,
        words_count: 0,
        bytes_count,
        is_binary: false,
        has_utf8_bom: false,
        elapsed_us: start.elapsed().as_micros() as u64,
    })
}

pub fn count_reader_detailed<R: Read>(
    reader: R,
    source_name: String,
    buffer_size_bytes: usize,
    logical_mode: bool,
    strict_utf8: bool,
) -> Result<FileStats, LineCountError> {
    let start = Instant::now();
    let mut buf_reader = BufReader::with_capacity(buffer_size_bytes, reader);
    let mut state = DetailedScanState::default();

    loop {
        let buffer = match buf_reader.fill_buf() {
            Ok(buf) => buf,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(LineCountError::from_io(source_name, e)),
        };

        let len = buffer.len();
        if len == 0 {
            break;
        }

        state.process_chunk(buffer, strict_utf8);
        buf_reader.consume(len);
    }

    let elapsed_us = start.elapsed().as_micros() as u64;
    state.finalize(source_name, logical_mode, strict_utf8, elapsed_us)
}

pub fn count_file_mmap(
    path: &Path,
    source_name: String,
    config: &CountConfig,
) -> Result<FileStats, LineCountError> {
    let start = Instant::now();
    let file = File::open(path).map_err(|e| LineCountError::from_io(&source_name, e))?;
    let metadata = file
        .metadata()
        .map_err(|e| LineCountError::from_io(&source_name, e))?;

    if metadata.len() == 0 {
        return Ok(FileStats {
            source_name,
            lines_newline: 0,
            lines_logical: 0,
            crlf_count: 0,
            lf_only_count: 0,
            empty_lines: 0,
            words_count: 0,
            bytes_count: 0,
            is_binary: false,
            has_utf8_bom: false,
            elapsed_us: start.elapsed().as_micros() as u64,
        });
    }

    // Безпечне відображення файлу у віртуальний адресний простір процесу
    let mmap = unsafe { MmapOptions::new().map(&file) }
        .map_err(|e| LineCountError::from_io(&source_name, e))?;
    let slice: &[u8] = &mmap[..];

    if config.detailed || config.strict_utf8 {
        let mut state = DetailedScanState::default();
        state.process_chunk(slice, config.strict_utf8);
        let elapsed_us = start.elapsed().as_micros() as u64;
        state.finalize(source_name, config.logical, config.strict_utf8, elapsed_us)
    } else {
        let lines_newline = memchr_iter(b'\n', slice).count() as u64;
        let bytes_count = slice.len() as u64;
        let has_unterminated = bytes_count > 0 && slice.last().copied() != Some(b'\n');
        let lines_logical = if has_unterminated {
            lines_newline + 1
        } else {
            lines_newline
        };

        Ok(FileStats {
            source_name,
            lines_newline,
            lines_logical,
            crlf_count: 0,
            lf_only_count: lines_newline,
            empty_lines: 0,
            words_count: 0,
            bytes_count,
            is_binary: false,
            has_utf8_bom: false,
            elapsed_us: start.elapsed().as_micros() as u64,
        })
    }
}

pub fn count_file(path: &Path, config: &CountConfig) -> Result<FileStats, LineCountError> {
    let source_name = path.display().to_string();
    let metadata = std::fs::metadata(path).map_err(|e| LineCountError::from_io(&source_name, e))?;

    if metadata.is_dir() {
        return Err(LineCountError::IsADirectory { path: source_name });
    }

    if config.use_mmap {
        return count_file_mmap(path, source_name, config);
    }

    let file = File::open(path).map_err(|e| LineCountError::from_io(&source_name, e))?;
    if config.detailed || config.strict_utf8 {
        count_reader_detailed(
            file,
            source_name,
            config.buffer_size_bytes,
            config.logical,
            config.strict_utf8,
        )
    } else {
        count_reader_fast(file, source_name, config.buffer_size_bytes)
    }
}
