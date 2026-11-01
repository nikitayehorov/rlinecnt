use crate::cli::CountConfig;
use crate::error::LineCountError;
use serde::Serialize;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileStats {
    pub source_name: String,
    pub lines_newline: u64,
    pub lines_logical: u64,
    pub crlf_count: u64,
    pub lf_only_count: u64,
    pub empty_lines: u64,
    pub words_count: u64,
    pub bytes_count: u64,
    pub is_binary: bool,
    pub has_utf8_bom: bool,
    pub elapsed_us: u64,
}

impl FileStats {
    pub fn format_label(&self) -> &'static str {
        if self.is_binary {
            "Binary(0x00)"
        } else if self.bytes_count == 0 {
            "Empty"
        } else if self.crlf_count > 0 && self.lf_only_count > 0 {
            "Mixed EOL"
        } else if self.has_utf8_bom && self.crlf_count > 0 {
            "UTF8-BOM/CRLF"
        } else if self.has_utf8_bom {
            "UTF8-BOM/LF"
        } else if self.crlf_count > 0 {
            "Text/CRLF"
        } else {
            "Text/LF"
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TotalSummary {
    pub total_files: usize,
    pub failed_files: usize,
    pub total_lines_newline: u64,
    pub total_lines_logical: u64,
    pub total_crlf: u64,
    pub total_lf_only: u64,
    pub total_empty_lines: u64,
    pub total_words: u64,
    pub total_bytes: u64,
    pub total_elapsed_us: u64,
}

#[derive(Debug, Default)]
pub struct BatchReport {
    pub files: Vec<FileStats>,
    pub errors: Vec<LineCountError>,
    pub summary: TotalSummary,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    files: &'a [FileStats],
    summary: &'a TotalSummary,
}

impl BatchReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_success(&mut self, stat: FileStats) {
        self.summary.total_files += 1;
        self.summary.total_lines_newline += stat.lines_newline;
        self.summary.total_lines_logical += stat.lines_logical;
        self.summary.total_crlf += stat.crlf_count;
        self.summary.total_lf_only += stat.lf_only_count;
        self.summary.total_empty_lines += stat.empty_lines;
        self.summary.total_words += stat.words_count;
        self.summary.total_bytes += stat.bytes_count;
        self.files.push(stat);
    }

    pub fn add_error(&mut self, err: LineCountError) {
        self.summary.failed_files += 1;
        self.errors.push(err);
    }

    pub fn render<W: Write>(&self, writer: &mut W, config: &CountConfig) -> io::Result<()> {
        if config.json {
            let payload = JsonReport {
                files: &self.files,
                summary: &self.summary,
            };
            let json_text = serde_json::to_string_pretty(&payload)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writeln!(writer, "{}", json_text)?;
            return Ok(());
        }

        if config.lines_only {
            if !config.summary_only {
                for f in &self.files {
                    let count = if config.logical {
                        f.lines_logical
                    } else {
                        f.lines_newline
                    };
                    if f.source_name == "<stdin>" && self.files.len() == 1 {
                        writeln!(writer, "{}", count)?;
                    } else {
                        writeln!(writer, "{:>8} {}", count, f.source_name)?;
                    }
                }
            }
            if self.files.len() > 1 || config.summary_only {
                let total = if config.logical {
                    self.summary.total_lines_logical
                } else {
                    self.summary.total_lines_newline
                };
                writeln!(writer, "{:>8} total", total)?;
            }
            return Ok(());
        }

        if config.detailed {
            self.render_detailed_table(writer, config)?;
        } else {
            self.render_standard_table(writer, config)?;
        }

        Ok(())
    }

    fn render_standard_table<W: Write>(&self, w: &mut W, config: &CountConfig) -> io::Result<()> {
        let sep = "-------------------------------------------------------------------------";
        let col_title = if config.logical {
            "Lines (Logic)"
        } else {
            "Lines (POSIX)"
        };
        writeln!(w, "{}", sep)?;
        writeln!(w, " {:>13} | {:>13} | {:>8} | Source", col_title, "Bytes", "Time")?;
        writeln!(w, "{}", sep)?;

        if !config.summary_only {
            for f in &self.files {
                let cnt = if config.logical {
                    f.lines_logical
                } else {
                    f.lines_newline
                };
                let ms = f.elapsed_us as f64 / 1000.0;
                writeln!(
                    w,
                    " {:>13} | {:>13} | {:>6.2}ms | {}",
                    cnt, f.bytes_count, ms, f.source_name
                )?;
            }
            if self.files.len() > 1 {
                writeln!(w, "{}", sep)?;
            }
        }

        if self.files.len() > 1 || config.summary_only {
            let total_cnt = if config.logical {
                self.summary.total_lines_logical
            } else {
                self.summary.total_lines_newline
            };
            let total_ms = self.summary.total_elapsed_us as f64 / 1000.0;
            writeln!(
                w,
                " {:>13} | {:>13} | {:>6.2}ms | TOTAL ({} files, {} failed)",
                total_cnt,
                self.summary.total_bytes,
                total_ms,
                self.summary.total_files,
                self.summary.failed_files
            )?;
        }
        writeln!(w, "{}", sep)?;
        Ok(())
    }

    fn render_detailed_table<W: Write>(&self, w: &mut W, config: &CountConfig) -> io::Result<()> {
        let sep = "=================================================================================================================";
        writeln!(w, "{}", sep)?;
        writeln!(
            w,
            " {:>11} | {:>8} | {:>8} | {:>8} | {:>8} | {:>8} | {:>10} | {:>10} | Source",
            "POSIX (\\n)", "Logical", "CRLF", "LF-only", "Empty", "Words", "Bytes", "Format"
        )?;
        writeln!(w, "{}", sep)?;

        if !config.summary_only {
            for f in &self.files {
                writeln!(
                    w,
                    " {:>11} | {:>8} | {:>8} | {:>8} | {:>8} | {:>8} | {:>10} | {:>10} | {}",
                    f.lines_newline,
                    f.lines_logical,
                    f.crlf_count,
                    f.lf_only_count,
                    f.empty_lines,
                    f.words_count,
                    f.bytes_count,
                    f.format_label(),
                    f.source_name
                )?;
            }
            if self.files.len() > 1 {
                writeln!(w, "{}", sep)?;
            }
        }

        if self.files.len() > 1 || config.summary_only {
            let total_ms = self.summary.total_elapsed_us as f64 / 1000.0;
            writeln!(
                w,
                " {:>11} | {:>8} | {:>8} | {:>8} | {:>8} | {:>8} | {:>10} | {:>10} | {} files ({:.2}ms)",
                self.summary.total_lines_newline,
                self.summary.total_lines_logical,
                self.summary.total_crlf,
                self.summary.total_lf_only,
                self.summary.total_empty_lines,
                self.summary.total_words,
                self.summary.total_bytes,
                "TOTAL",
                self.summary.total_files,
                total_ms
            )?;
        }
        writeln!(w, "{}", sep)?;
        Ok(())
    }
}
