use std::fmt;
use std::io::{self, ErrorKind};

#[derive(Debug)]
pub enum LineCountError {
    FileNotFound { path: String },
    PermissionDenied { path: String },
    IsADirectory { path: String },
    InvalidBufferSize { provided_kb: usize },
    InvalidUtf8OrBinary { path: String, reason: String },
    DirectoryTraversalError { path: String, message: String },
    IoError { path: String, source: io::Error },
}

impl LineCountError {
    pub fn from_io(path: impl Into<String>, err: io::Error) -> Self {
        let path_str = path.into();
        match err.kind() {
            ErrorKind::NotFound => LineCountError::FileNotFound { path: path_str },
            ErrorKind::PermissionDenied => LineCountError::PermissionDenied { path: path_str },
            _ => LineCountError::IoError {
                path: path_str,
                source: err,
            },
        }
    }
}

impl fmt::Display for LineCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineCountError::FileNotFound { path } => {
                write!(f, "Помилка [Не знайдено]: файл або каталог '{}' не існує.", path)
            }
            LineCountError::PermissionDenied { path } => {
                write!(
                    f,
                    "Помилка [Відмовлено у доступі]: недостатньо прав для читання '{}'.",
                    path
                )
            }
            LineCountError::IsADirectory { path } => {
                write!(
                    f,
                    "Помилка [Це директорія]: '{}' є каталогом (використайте ключ -r / --recursive для обходу).",
                    path
                )
            }
            LineCountError::InvalidBufferSize { provided_kb } => {
                write!(
                    f,
                    "Помилка [Некоректний буфер]: розмір буфера {} КБ виходить за допустимі межі (4..16384 КБ).",
                    provided_kb
                )
            }
            LineCountError::InvalidUtf8OrBinary { path, reason } => {
                write!(
                    f,
                    "Помилка [Некоректне кодування або бінарні дані]: файл '{}' не є валідним UTF-8 ({}).",
                    path, reason
                )
            }
            LineCountError::DirectoryTraversalError { path, message } => {
                write!(
                    f,
                    "Помилка [Обхід каталогу]: не вдалося прочитати елемент у '{}': {}.",
                    path, message
                )
            }
            LineCountError::IoError { path, source } => {
                write!(
                    f,
                    "Помилка [Введення/виведення]: збій під час читання '{}': {}.",
                    path, source
                )
            }
        }
    }
}

impl std::error::Error for LineCountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LineCountError::IoError { source, .. } => Some(source),
            _ => None,
        }
    }
}
