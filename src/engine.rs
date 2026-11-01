use crate::cli::CountConfig;
use crate::counter::{count_file, count_reader_detailed, count_reader_fast};
use crate::error::LineCountError;
use crate::stats::{BatchReport, FileStats};
use rayon::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub enum InputTarget {
    Stdin,
    FilePath(PathBuf),
}

pub fn collect_targets(
    raw_paths: Vec<PathBuf>,
    config: &CountConfig,
) -> (Vec<InputTarget>, Vec<LineCountError>) {
    let mut targets = Vec::new();
    let mut pre_errors = Vec::new();

    if raw_paths.is_empty() {
        targets.push(InputTarget::Stdin);
        return (targets, pre_errors);
    }

    for path in raw_paths {
        if path.as_os_str() == "-" {
            targets.push(InputTarget::Stdin);
            continue;
        }

        match std::fs::metadata(&path) {
            Ok(meta) => {
                if meta.is_dir() {
                    if !config.recursive {
                        pre_errors.push(LineCountError::IsADirectory {
                            path: path.display().to_string(),
                        });
                    } else {
                        for entry_res in WalkDir::new(&path).follow_links(false) {
                            match entry_res {
                                Ok(entry) => {
                                    if entry.file_type().is_file() {
                                        targets.push(InputTarget::FilePath(entry.into_path()));
                                    }
                                }
                                Err(walk_err) => {
                                    let err_path = walk_err
                                        .path()
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|| path.display().to_string());
                                    pre_errors.push(LineCountError::DirectoryTraversalError {
                                        path: err_path,
                                        message: walk_err.to_string(),
                                    });
                                }
                            }
                        }
                    }
                } else {
                    targets.push(InputTarget::FilePath(path));
                }
            }
            Err(io_err) => {
                pre_errors.push(LineCountError::from_io(path.display().to_string(), io_err));
            }
        }
    }

    (targets, pre_errors)
}

fn process_target(target: InputTarget, config: &CountConfig) -> Result<FileStats, LineCountError> {
    match target {
        InputTarget::Stdin => {
            let stdin = io::stdin();
            let handle = stdin.lock();
            if config.detailed || config.strict_utf8 {
                count_reader_detailed(
                    handle,
                    String::from("<stdin>"),
                    config.buffer_size_bytes,
                    config.logical,
                    config.strict_utf8,
                )
            } else {
                count_reader_fast(
                    handle,
                    String::from("<stdin>"),
                    config.buffer_size_bytes,
                )
            }
        }
        InputTarget::FilePath(path) => count_file(&path, config),
    }
}

pub fn run_engine(raw_paths: Vec<PathBuf>, config: &CountConfig) -> BatchReport {
    let start_total = Instant::now();
    let (targets, pre_errors) = collect_targets(raw_paths, config);
    let mut report = BatchReport::new();

    for err in pre_errors {
        report.add_error(err);
    }

    let has_stdin = targets.iter().any(|t| matches!(t, InputTarget::Stdin));

    let results: Vec<Result<FileStats, LineCountError>> = if has_stdin || targets.len() <= 1 {
        targets
            .into_iter()
            .map(|t| process_target(t, config))
            .collect()
    } else {
        let mut builder = rayon::ThreadPoolBuilder::new();
        if config.threads > 0 {
            builder = builder.num_threads(config.threads);
        }
        match builder.build() {
            Ok(pool) => pool.install(|| {
                targets
                    .into_par_iter()
                    .map(|t| process_target(t, config))
                    .collect()
            }),
            Err(_) => targets
                .into_par_iter()
                .map(|t| process_target(t, config))
                .collect(),
        }
    };

    for res in results {
        match res {
            Ok(stat) => report.add_success(stat),
            Err(err) => report.add_error(err),
        }
    }

    report.summary.total_elapsed_us = start_total.elapsed().as_micros() as u64;
    report
}
