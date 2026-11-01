use crate::error::LineCountError;
use clap::Parser;
use std::path::PathBuf;

pub const MIN_BUFFER_KB: usize = 4;
pub const MAX_BUFFER_KB: usize = 16_384;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "rlinecnt",
    author = "Єгоров Н. Р., група КОФ201ДОФ23",
    version = "1.0.0",
    about = "Кросплатформна високопродуктивна системна утиліта підрахунку рядків (Варіант 20)",
    long_about = "Системна утиліта мовою Rust для швидкісного поблочного та векторизованого (SIMD) \
                  підрахунку рядків у текстових файлах і потоках стандартного введення (stdin) \
                  у середовищах Windows, Linux та macOS."
)]
pub struct CliArgs {
    /// Шляхи до файлів або директорій (за замовчуванням або при '-' читається stdin)
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Виводити лише кількість рядків (режим сумісності зі стандартним wc -l)
    #[arg(short = 'l', long = "lines-only")]
    pub lines_only: bool,

    /// Рекурсивно обходити всі вкладені каталоги
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,

    /// Кількість потоків у пулі паралельної обробки (0 = автоматично за ядрами ЦП)
    #[arg(short = 't', long = "threads", default_value_t = 0)]
    pub threads: usize,

    /// Розмір буфера читання I/O у кілобайтах (від 4 до 16384 КБ)
    #[arg(short = 'b', long = "buffer-size", default_value_t = 64)]
    pub buffer_size_kb: usize,

    /// Використовувати відображення файлів у віртуальну пам'ять (mmap)
    #[arg(short = 'm', long = "mmap")]
    pub use_mmap: bool,

    /// Враховувати останній непорожній рядок без завершального символу '\n'
    #[arg(short = 'L', long = "logical")]
    pub logical: bool,

    /// Виводити розширену статистику (LF, CRLF, порожні рядки, слова, байти, BOM)
    #[arg(short = 'd', long = "detailed")]
    pub detailed: bool,

    /// Суворо перевіряти кодування UTF-8 та відхиляти бінарні або не-UTF-8 файли
    #[arg(short = 'u', long = "strict-utf8")]
    pub strict_utf8: bool,

    /// Виводити звіт у структурованому форматі JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,

    /// Виводити лише підсумковий рядок TOTAL при пакетній обробці
    #[arg(short = 's', long = "summary-only")]
    pub summary_only: bool,
}

#[derive(Debug, Clone)]
pub struct CountConfig {
    pub lines_only: bool,
    pub recursive: bool,
    pub threads: usize,
    pub buffer_size_bytes: usize,
    pub use_mmap: bool,
    pub logical: bool,
    pub detailed: bool,
    pub strict_utf8: bool,
    pub json: bool,
    pub summary_only: bool,
}

impl CliArgs {
    pub fn into_config(self) -> Result<(Vec<PathBuf>, CountConfig), LineCountError> {
        if self.buffer_size_kb < MIN_BUFFER_KB || self.buffer_size_kb > MAX_BUFFER_KB {
            return Err(LineCountError::InvalidBufferSize {
                provided_kb: self.buffer_size_kb,
            });
        }

        let config = CountConfig {
            lines_only: self.lines_only,
            recursive: self.recursive,
            threads: self.threads,
            buffer_size_bytes: self.buffer_size_kb * 1024,
            use_mmap: self.use_mmap,
            logical: self.logical,
            detailed: self.detailed,
            strict_utf8: self.strict_utf8,
            json: self.json,
            summary_only: self.summary_only,
        };

        Ok((self.paths, config))
    }
}
