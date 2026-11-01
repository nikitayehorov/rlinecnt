use rlinecnt::cli::{CliArgs, CountConfig};
use rlinecnt::counter::{count_file, count_reader_detailed, count_reader_fast};
use rlinecnt::engine::run_engine;
use rlinecnt::error::LineCountError;
use std::io::Cursor;
use std::path::PathBuf;

fn default_config() -> CountConfig {
    CountConfig {
        lines_only: false,
        recursive: false,
        threads: 2,
        buffer_size_bytes: 64 * 1024,
        use_mmap: false,
        logical: false,
        detailed: true,
        strict_utf8: false,
        json: false,
        summary_only: false,
    }
}

#[test]
fn test_01_empty_stream() {
    let data: &[u8] = b"";
    let stats = count_reader_fast(Cursor::new(data), "empty".into(), 4096).unwrap();
    assert_eq!(stats.lines_newline, 0);
    assert_eq!(stats.lines_logical, 0);
    assert_eq!(stats.bytes_count, 0);
}

#[test]
fn test_02_single_newline() {
    let data: &[u8] = b"\n";
    let stats = count_reader_detailed(Cursor::new(data), "nl".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 1);
    assert_eq!(stats.lines_logical, 1);
    assert_eq!(stats.empty_lines, 1);
    assert_eq!(stats.words_count, 0);
}

#[test]
fn test_03_single_char_no_newline() {
    let data: &[u8] = b"A";
    let stats = count_reader_detailed(Cursor::new(data), "char".into(), 4096, true, false).unwrap();
    assert_eq!(stats.lines_newline, 0);
    assert_eq!(stats.lines_logical, 1);
    assert_eq!(stats.words_count, 1);
}

#[test]
fn test_04_unix_lf_lines() {
    let data: &[u8] = b"alpha\nbeta\ngamma\n";
    let stats = count_reader_detailed(Cursor::new(data), "lf".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 3);
    assert_eq!(stats.lf_only_count, 3);
    assert_eq!(stats.crlf_count, 0);
    assert_eq!(stats.words_count, 3);
}

#[test]
fn test_05_windows_crlf_lines() {
    let data: &[u8] = b"first\r\nsecond\r\nthird\r\n";
    let stats =
        count_reader_detailed(Cursor::new(data), "crlf".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 3);
    assert_eq!(stats.crlf_count, 3);
    assert_eq!(stats.lf_only_count, 0);
    assert_eq!(stats.format_label(), "Text/CRLF");
}

#[test]
fn test_06_mixed_lf_and_crlf() {
    let data: &[u8] = b"win\r\nunix\nwin2\r\n";
    let stats =
        count_reader_detailed(Cursor::new(data), "mixed".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 3);
    assert_eq!(stats.crlf_count, 2);
    assert_eq!(stats.lf_only_count, 1);
    assert_eq!(stats.format_label(), "Mixed EOL");
}

#[test]
fn test_07_unterminated_last_line() {
    let data: &[u8] = b"line_one\nline_two_no_eol";
    let stats = count_reader_fast(Cursor::new(data), "unterm".into(), 4096).unwrap();
    assert_eq!(stats.lines_newline, 1);
    assert_eq!(stats.lines_logical, 2);
}

#[test]
fn test_08_crlf_split_across_chunk_boundary() {
    // Буфер розміром 4 байти: "abc\r" (4 байти) + "\ndef\n" (5 байтів)
    let data: &[u8] = b"abc\r\ndef\n";
    let stats =
        count_reader_detailed(Cursor::new(data), "split_crlf".into(), 4, false, false).unwrap();
    assert_eq!(stats.lines_newline, 2);
    assert_eq!(stats.crlf_count, 1);
    assert_eq!(stats.lf_only_count, 1);
}

#[test]
fn test_09_word_split_across_chunk_boundary() {
    // Слово "systems" розірване між двома буферами по 4 байти ("syst" + "ems\n")
    let data: &[u8] = b"systems\n";
    let stats =
        count_reader_detailed(Cursor::new(data), "split_word".into(), 4, false, false).unwrap();
    assert_eq!(stats.words_count, 1);
    assert_eq!(stats.lines_newline, 1);
}

#[test]
fn test_10_empty_and_whitespace_lines() {
    let data: &[u8] = b"\n   \n\t\r\nreal text\n";
    let stats =
        count_reader_detailed(Cursor::new(data), "empty_ws".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 4);
    assert_eq!(stats.empty_lines, 3);
    assert_eq!(stats.words_count, 2);
}

#[test]
fn test_11_utf8_ukrainian_cyrillic() {
    let text = "Системне програмування\nМова Rust\nЄвропейський університет\n";
    let stats =
        count_reader_detailed(Cursor::new(text.as_bytes()), "ukr".into(), 4096, false, false)
            .unwrap();
    assert_eq!(stats.lines_newline, 3);
    assert_eq!(stats.words_count, 6);
    assert_eq!(stats.bytes_count, text.len() as u64);
}

#[test]
fn test_12_binary_file_detection() {
    let data: &[u8] = b"header\n\x00\xFF\xFEbinary_payload\n";
    let stats = count_reader_detailed(Cursor::new(data), "bin".into(), 4096, false, false).unwrap();
    assert_eq!(stats.lines_newline, 2);
    assert!(stats.is_binary);
    assert_eq!(stats.format_label(), "Binary(0x00)");
}

#[test]
fn test_13_mmap_matches_buffered_reader() {
    let cargo_path = PathBuf::from("Cargo.toml");
    if cargo_path.exists() {
        let mut cfg_buf = default_config();
        cfg_buf.use_mmap = false;
        let mut cfg_mmap = default_config();
        cfg_mmap.use_mmap = true;

        let s1 = count_file(&cargo_path, &cfg_buf).unwrap();
        let s2 = count_file(&cargo_path, &cfg_mmap).unwrap();
        assert_eq!(s1.lines_newline, s2.lines_newline);
        assert_eq!(s1.bytes_count, s2.bytes_count);
        assert_eq!(s1.words_count, s2.words_count);
    }
}

#[test]
fn test_14_missing_file_returns_not_found() {
    let cfg = default_config();
    let res = count_file(&PathBuf::from("non_existent_file_12345.log"), &cfg);
    assert!(matches!(res, Err(LineCountError::FileNotFound { .. })));
}

#[test]
fn test_15_directory_without_recursive_returns_error() {
    let mut cfg = default_config();
    cfg.recursive = false;
    let report = run_engine(vec![PathBuf::from("src")], &cfg);
    assert_eq!(report.summary.failed_files, 1);
    assert!(matches!(
        report.errors.first(),
        Some(LineCountError::IsADirectory { .. })
    ));
}

#[test]
fn test_16_recursive_directory_walk_succeeds() {
    let mut cfg = default_config();
    cfg.recursive = true;
    let report = run_engine(vec![PathBuf::from("src")], &cfg);
    assert_eq!(report.summary.failed_files, 0);
    assert!(report.summary.total_files >= 5);
    assert!(report.summary.total_lines_newline > 100);
}

#[test]
fn test_17_invalid_buffer_size_rejected() {
    let args = CliArgs {
        paths: vec![],
        lines_only: false,
        recursive: false,
        threads: 0,
        buffer_size_kb: 1, // Менше допустимого мінімуму 4 КБ
        use_mmap: false,
        logical: false,
        detailed: false,
        strict_utf8: false,
        json: false,
        summary_only: false,
    };
    assert!(matches!(
        args.into_config(),
        Err(LineCountError::InvalidBufferSize { provided_kb: 1 })
    ));
}

#[test]
fn test_18_json_output_rendering() {
    let mut cfg = default_config();
    cfg.json = true;
    let report = run_engine(vec![PathBuf::from("Cargo.toml")], &cfg);
    let mut output = Vec::new();
    report.render(&mut output, &cfg).unwrap();
    let json_str = String::from_utf8(output).unwrap();
    assert!(json_str.contains("\"total_files\": 1"));
    assert!(json_str.contains("\"lines_newline\":"));
}

#[test]
fn test_19_utf8_bom_detection_and_handling() {
    // Префікс UTF-8 BOM (0xEF 0xBB 0xBF) + порожній рядок "\n"
    let data: &[u8] = b"\xEF\xBB\xBF\n";
    let stats = count_reader_detailed(Cursor::new(data), "bom".into(), 4096, false, true).unwrap();
    assert!(stats.has_utf8_bom);
    assert_eq!(stats.lines_newline, 1);
    assert_eq!(stats.empty_lines, 1);
    assert_eq!(stats.words_count, 0);
    assert_eq!(stats.format_label(), "UTF8-BOM/LF");
}

#[test]
fn test_20_strict_utf8_rejects_invalid_bytes() {
    // Невалідна послідовність UTF-8 / UTF-16LE BOM при увімкненому strict_utf8 = true
    let data: &[u8] = b"\xFF\xFEinvalid_utf8\n";
    let res = count_reader_detailed(Cursor::new(data), "bad_utf8".into(), 4096, false, true);
    assert!(matches!(
        res,
        Err(LineCountError::InvalidUtf8OrBinary { .. })
    ));
}
