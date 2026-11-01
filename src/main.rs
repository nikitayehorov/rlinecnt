use clap::Parser;
use rlinecnt::cli::CliArgs;
use rlinecnt::engine::run_engine;
use std::io::{self, BufWriter, ErrorKind, Write};
use std::process::ExitCode;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_PARTIAL_OR_IO_ERROR: u8 = 1;
pub const EXIT_CLI_USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    let args = CliArgs::parse();

    let (paths, config) = match args.into_config() {
        Ok(valid) => valid,
        Err(cfg_err) => {
            let _ = writeln!(io::stderr(), "{}", cfg_err);
            return ExitCode::from(EXIT_CLI_USAGE_ERROR);
        }
    };

    let report = run_engine(paths, &config);

    // Виведення всіх помилок у стандартний потік помилок (stderr)
    for err in &report.errors {
        let _ = writeln!(io::stderr(), "{}", err);
    }

    // Виведення таблиці або результатів у стандартний потік виведення (stdout)
    if !report.files.is_empty() || config.json {
        let stdout = io::stdout();
        let mut handle = BufWriter::new(stdout.lock());
        if let Err(write_err) = report.render(&mut handle, &config) {
            if write_err.kind() == ErrorKind::BrokenPipe {
                // Коректне завершення при закритті конвеєра наступним процесом (напр. head)
                return ExitCode::from(EXIT_SUCCESS);
            }
            let _ = writeln!(io::stderr(), "Помилка запису у потік виведення: {}", write_err);
            return ExitCode::from(EXIT_PARTIAL_OR_IO_ERROR);
        }
        let _ = handle.flush();
    }

    if !report.errors.is_empty() {
        ExitCode::from(EXIT_PARTIAL_OR_IO_ERROR)
    } else {
        ExitCode::from(EXIT_SUCCESS)
    }
}
