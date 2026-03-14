use crashlog_collector::collector::{
    DEFAULT_MAX_WORDS_PER_CHUNK, MO2_FILENAME, ProgressEvent, RunConfig,
    format_python_local_timestamp, run_collection_sync,
};
use filetime::{FileTime, set_file_mtime};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

#[test]
fn normal_session_matches_golden_outputs() {
    let logs_dir = tempdir().unwrap();
    let mo2_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let newest = write_text_with_mtime(
        logs_dir.path(),
        "crash_002.log",
        "newest line one\nnewest line two\n",
        1_700_000_000,
    );
    let within = write_text_with_mtime(
        logs_dir.path(),
        "crash_001.log",
        "older session line\n",
        1_700_000_000 - (20 * 60),
    );
    write_text_with_mtime(
        logs_dir.path(),
        "crash_old.log",
        "too old to include\n",
        1_700_000_000 - (45 * 60),
    );

    fs::write(
        mo2_dir.path().join("profile.ini"),
        "[General]\nsLanguage=ENGLISH\n",
    )
    .unwrap();

    let mut events = Vec::new();
    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: None,
            mo2_dir: Some(mo2_dir.path().to_path_buf()),
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |event| events.push(event),
    );

    assert_eq!(summary.logs_processed, 2);
    assert_eq!(summary.log_chunks_generated, 1);
    assert_eq!(summary.mo2_files_processed, 1);
    assert!(summary.warnings.is_empty());
    assert!(events.iter().any(|event| matches!(event, ProgressEvent::Status(message) if message.contains("Found 2 log source files"))));

    let logs_output = fs::read_to_string(output_dir.path().join("LOGS_part_1.txt")).unwrap();
    let expected_logs = render_fixture(
        "normal/LOGS_part_1.txt",
        &[
            ("{{NEWEST_TS}}", &format_python_local_timestamp(newest)),
            ("{{WITHIN_TS}}", &format_python_local_timestamp(within)),
        ],
    );
    assert_eq!(logs_output, expected_logs);

    let mo2_output = fs::read_to_string(output_dir.path().join(MO2_FILENAME)).unwrap();
    assert_eq!(mo2_output, render_fixture("normal/MO2 PROFILE.txt", &[]));
}

#[test]
fn empty_logs_folder_skips_log_output_but_keeps_mo2_step() {
    let logs_dir = tempdir().unwrap();
    let mo2_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: None,
            mo2_dir: Some(mo2_dir.path().to_path_buf()),
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.logs_processed, 0);
    assert_eq!(summary.log_chunks_generated, 0);
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.message.contains("No valid log files found"))
    );
    assert!(!output_dir.path().join("LOGS_part_1.txt").exists());

    let mo2_output = fs::read_to_string(output_dir.path().join(MO2_FILENAME)).unwrap();
    assert_eq!(mo2_output, "No text or ini files found in selected folder.");
}

#[test]
fn empty_mo2_folder_writes_fallback_file() {
    let mo2_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: None,
            overwrite_plugins_dir: None,
            mo2_dir: Some(mo2_dir.path().to_path_buf()),
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.mo2_files_processed, 0);
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.message.contains("Skipping logs"))
    );

    let mo2_output = fs::read_to_string(output_dir.path().join(MO2_FILENAME)).unwrap();
    assert_eq!(mo2_output, "No text or ini files found in selected folder.");
}

#[test]
fn invalid_utf8_is_replaced_lossily() {
    let mo2_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    fs::write(
        mo2_dir.path().join("invalid.txt"),
        [
            b'm', b'o', b'd', b'l', b'i', b's', b't', b'=', 0xFF, 0xFE, b'S', b'k', b'y', b'r',
            b'i', b'm', b'\n',
        ],
    )
    .unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: None,
            overwrite_plugins_dir: None,
            mo2_dir: Some(mo2_dir.path().to_path_buf()),
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.mo2_files_processed, 1);
    let mo2_output = fs::read_to_string(output_dir.path().join(MO2_FILENAME)).unwrap();
    assert_eq!(
        mo2_output,
        render_fixture("invalid_utf8/MO2 PROFILE.txt", &[])
    );
}

#[test]
fn invalid_log_folder_still_allows_mo2_export() {
    let missing_logs_dir = tempdir().unwrap().path().join("missing");
    let mo2_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    fs::write(mo2_dir.path().join("settings.txt"), "Only MO2 ran.\n").unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(missing_logs_dir),
            overwrite_plugins_dir: None,
            mo2_dir: Some(mo2_dir.path().to_path_buf()),
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.logs_processed, 0);
    assert_eq!(summary.mo2_files_processed, 1);
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.message.contains("No valid log directory selected"))
    );

    let mo2_output = fs::read_to_string(output_dir.path().join(MO2_FILENAME)).unwrap();
    assert_eq!(
        mo2_output,
        render_fixture("missing_logs/MO2 PROFILE.txt", &[])
    );
}

#[test]
fn rerun_preserves_stale_extra_chunk_files() {
    let logs_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    write_text_with_mtime(
        logs_dir.path(),
        "crash_001.log",
        "fresh run\n",
        1_700_100_000,
    );
    fs::write(
        output_dir.path().join("LOGS_part_3.txt"),
        "stale extra chunk",
    )
    .unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: None,
            mo2_dir: None,
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.log_chunks_generated, 1);
    assert_eq!(
        fs::read_to_string(output_dir.path().join("LOGS_part_3.txt")).unwrap(),
        "stale extra chunk"
    );
    assert!(output_dir.path().join("LOGS_part_1.txt").exists());
}

#[test]
fn overwrite_plugins_folder_contributes_additional_log_sources() {
    let logs_dir = tempdir().unwrap();
    let overwrite_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let newest = write_text_with_mtime(
        overwrite_dir.path(),
        "plugin.JSONL",
        "{\"event\":\"boom\"}\n",
        1_700_200_000,
    );
    let within = write_text_with_mtime(
        logs_dir.path(),
        "skse.log",
        "skse line\n",
        1_700_200_000 - (15 * 60),
    );
    let also_within = write_text_with_mtime(
        overwrite_dir.path(),
        "stack.DMP",
        "dump payload\n",
        1_700_200_000 - (5 * 60),
    );
    write_text_with_mtime(
        overwrite_dir.path(),
        "ignore.ini",
        "should be ignored\n",
        1_700_200_000,
    );

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: Some(overwrite_dir.path().to_path_buf()),
            mo2_dir: None,
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.logs_processed, 3);
    let logs_output = fs::read_to_string(output_dir.path().join("LOGS_part_1.txt")).unwrap();

    assert!(logs_output.contains(&format_python_local_timestamp(newest)));
    assert!(logs_output.contains(&format_python_local_timestamp(within)));
    assert!(logs_output.contains(&format_python_local_timestamp(also_within)));
    assert!(logs_output.contains("plugin.JSONL"));
    assert!(logs_output.contains("skse.log"));
    assert!(logs_output.contains("stack.DMP"));
    assert!(!logs_output.contains("ignore.ini"));
}

#[test]
fn chunk_limit_truncates_log_output_and_emits_warning() {
    let logs_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    write_text_with_mtime(
        logs_dir.path(),
        "huge.log",
        "one two three four five six seven eight nine ten eleven twelve\n",
        1_700_300_000,
    );

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: None,
            mo2_dir: None,
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: 6,
            max_chunks: Some(2),
        },
        |_| {},
    );

    assert_eq!(summary.log_chunks_generated, 2);
    assert!(summary.warnings.iter().any(|warning| {
        warning
            .message
            .contains("Remaining log content was omitted")
    }));

    for chunk_name in ["LOGS_part_1.txt", "LOGS_part_2.txt"] {
        let chunk = fs::read_to_string(output_dir.path().join(chunk_name)).unwrap();
        assert!(
            crashlog_collector::collector::count_words(&chunk) <= 6,
            "{chunk_name} exceeded the configured word limit"
        );
    }
}

#[test]
fn punctuation_heavy_json_logs_still_honor_chunk_limit() {
    let logs_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    write_text_with_mtime(
        logs_dir.path(),
        "json.log",
        "{\"event\":\"boom\",\"plugin\":\"skse\",\"code\":42,\"phase\":\"load\",\"detail\":\"alpha\"}\n",
        1_700_400_000,
    );

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: Some(logs_dir.path().to_path_buf()),
            overwrite_plugins_dir: None,
            mo2_dir: None,
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: 12,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.log_chunks_generated, 2);

    for chunk_name in ["LOGS_part_1.txt", "LOGS_part_2.txt"] {
        let chunk = fs::read_to_string(output_dir.path().join(chunk_name)).unwrap();
        assert!(
            crashlog_collector::collector::count_words(&chunk) <= 12,
            "{chunk_name} exceeded the configured word limit"
        );
    }
}

#[test]
fn binary_like_dump_output_is_sanitized() {
    let overwrite_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    fs::write(
        overwrite_dir.path().join("hang.dmp"),
        b"C\0:\0\\\0G\0a\0m\0e\0s\0\\\0S\0k\0y\0r\0i\0m\0S\0E\0.\0e\0x\0e\0\x01\x02\r\n",
    )
    .unwrap();

    let summary = run_collection_sync(
        &RunConfig {
            log_dir: None,
            overwrite_plugins_dir: Some(overwrite_dir.path().to_path_buf()),
            mo2_dir: None,
            output_dir: output_dir.path().to_path_buf(),
            max_words_per_chunk: DEFAULT_MAX_WORDS_PER_CHUNK,
            max_chunks: None,
        },
        |_| {},
    );

    assert_eq!(summary.logs_processed, 1);
    let output = fs::read_to_string(output_dir.path().join("LOGS_part_1.txt")).unwrap();
    assert!(output.contains("C:\\Games\\SkyrimSE.exe"));
    assert!(!output.contains('\0'));
}

fn write_text_with_mtime(dir: &Path, name: &str, content: &str, unix_seconds: i64) -> SystemTime {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    let file_time = FileTime::from_unix_time(unix_seconds, 0);
    set_file_mtime(&path, file_time).unwrap();
    SystemTime::UNIX_EPOCH + Duration::from_secs(unix_seconds as u64)
}

fn render_fixture(path: &str, replacements: &[(&str, &str)]) -> String {
    let absolute = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path);
    let template = fs::read_to_string(absolute).unwrap();
    let mut normalized = template.replace("\r\n", "\n").replace('\n', "\r\n");

    if path.ends_with("MO2 PROFILE.txt") && normalized.ends_with("\r\n\r\n") {
        normalized.push_str("\r\n");
    }

    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }

    normalized
}
