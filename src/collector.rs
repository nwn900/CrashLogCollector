use chrono::{DateTime, Local, TimeZone};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime};

pub const DEFAULT_MAX_WORDS_PER_CHUNK: usize = 450_000;
pub const LOG_FILENAME_PREFIX: &str = "LOGS_part_";
pub const LOG_FILENAME_SUFFIX: &str = ".txt";
pub const MO2_FILENAME: &str = "MO2 PROFILE.txt";
const WINDOWS_NEWLINE: &str = "\r\n";
const PRIMARY_LOG_EXTENSIONS: &[&str] = &["log"];
const OVERWRITE_PLUGIN_LOG_EXTENSIONS: &[&str] = &["txt", "log", "json", "dmp", "jsonl"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub log_dir: Option<PathBuf>,
    pub overwrite_plugins_dir: Option<PathBuf>,
    pub mo2_dir: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub max_words_per_chunk: usize,
    pub max_chunks: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorWarning {
    pub step: StepKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Logs,
    Mo2,
    General,
}

impl StepKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Logs => "Logs",
            Self::Mo2 => "MO2",
            Self::General => "General",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub logs_processed: usize,
    pub log_chunks_generated: usize,
    pub mo2_files_processed: usize,
    pub warnings: Vec<CollectorWarning>,
    pub output_files: Vec<PathBuf>,
}

impl RunSummary {
    fn new() -> Self {
        Self {
            logs_processed: 0,
            log_chunks_generated: 0,
            mo2_files_processed: 0,
            warnings: Vec::new(),
            output_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    Status(String),
    Warning(CollectorWarning),
    Finished(RunSummary),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogCandidate {
    path: PathBuf,
    name: String,
    modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogDocument {
    name: String,
    modified: SystemTime,
    content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogWriteOutcome {
    paths: Vec<PathBuf>,
    truncated: bool,
}

pub fn default_skyrim_logs_dir() -> Option<PathBuf> {
    dirs::document_dir().map(|documents| {
        documents
            .join("My Games")
            .join("Skyrim Special Edition")
            .join("SKSE")
    })
}

pub fn default_output_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn spawn_collection_thread(
    config: RunConfig,
    sender: Sender<ProgressEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let summary = run_collection_sync(&config, |event| {
            let _ = sender.send(event);
        });
        let _ = sender.send(ProgressEvent::Finished(summary));
    })
}

pub fn run_collection_sync<F>(config: &RunConfig, mut on_event: F) -> RunSummary
where
    F: FnMut(ProgressEvent),
{
    let mut summary = RunSummary::new();

    emit_status(
        &mut on_event,
        format!("Output folder: {}", config.output_dir.display()),
    );

    if let Err(error) = fs::create_dir_all(&config.output_dir) {
        push_warning(
            &mut summary,
            &mut on_event,
            StepKind::General,
            format!(
                "Failed to create output directory '{}': {error}",
                config.output_dir.display()
            ),
        );
        return summary;
    }

    process_logs(config, &mut summary, &mut on_event);
    process_mo2(config, &mut summary, &mut on_event);
    emit_status(&mut on_event, "Operation complete.".to_owned());

    summary
}

pub fn count_words(text: &str) -> usize {
    let mut words = 0usize;
    let mut in_word = false;

    for character in text.chars() {
        if is_word_char(character) {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }

    words
}

pub fn filter_recent_logs(mut candidates: Vec<LogCandidate>) -> Vec<LogCandidate> {
    candidates.sort_by(|left, right| right.modified.cmp(&left.modified));

    let Some(newest) = candidates.first().map(|candidate| candidate.modified) else {
        return Vec::new();
    };

    let cutoff = newest
        .checked_sub(Duration::from_secs(30 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    candidates
        .into_iter()
        .filter(|candidate| candidate.modified >= cutoff)
        .collect()
}

pub fn format_python_datetime<Tz>(datetime: DateTime<Tz>) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let base = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
    let microseconds = datetime.timestamp_subsec_micros();

    if microseconds == 0 {
        base
    } else {
        format!("{base}.{microseconds:06}")
    }
}

pub fn format_python_local_timestamp(system_time: SystemTime) -> String {
    let datetime: DateTime<Local> = system_time.into();
    format_python_datetime(datetime)
}

fn process_logs<F>(config: &RunConfig, summary: &mut RunSummary, on_event: &mut F)
where
    F: FnMut(ProgressEvent),
{
    emit_status(
        on_event,
        "--- Step 1: Skyrim Logs Collection ---".to_owned(),
    );

    let mut candidates = Vec::new();
    let mut valid_source_count = 0usize;

    if let Some(log_dir) = normalize_optional_dir(&config.log_dir) {
        emit_status(
            on_event,
            format!("Selected log folder: {}", log_dir.display()),
        );
        if let Some(source_candidates) = collect_log_candidates(
            &log_dir,
            PRIMARY_LOG_EXTENSIONS,
            StepKind::Logs,
            summary,
            on_event,
        ) {
            valid_source_count += 1;
            candidates.extend(source_candidates);
        }
    } else if config.log_dir.is_some() {
        push_warning(
            summary,
            on_event,
            StepKind::Logs,
            "No valid log directory selected. Skipping Skyrim logs.".to_owned(),
        );
    }

    if let Some(overwrite_plugins_dir) = normalize_optional_dir(&config.overwrite_plugins_dir) {
        emit_status(
            on_event,
            format!(
                "Selected overwrite plugins folder: {}",
                overwrite_plugins_dir.display()
            ),
        );
        if let Some(source_candidates) = collect_log_candidates(
            &overwrite_plugins_dir,
            OVERWRITE_PLUGIN_LOG_EXTENSIONS,
            StepKind::Logs,
            summary,
            on_event,
        ) {
            valid_source_count += 1;
            candidates.extend(source_candidates);
        }
    } else if is_non_empty_path(&config.overwrite_plugins_dir) {
        push_warning(
            summary,
            on_event,
            StepKind::Logs,
            "No valid overwrite plugins directory selected. Skipping overwrite plugin logs."
                .to_owned(),
        );
    }

    if valid_source_count == 0 {
        push_warning(
            summary,
            on_event,
            StepKind::Logs,
            "No valid log source directory selected. Skipping logs.".to_owned(),
        );
        return;
    }

    if candidates.is_empty() {
        push_warning(
            summary,
            on_event,
            StepKind::Logs,
            "No valid log files found in the selected log source folders.".to_owned(),
        );
        return;
    }

    let recent_logs = filter_recent_logs(candidates);
    emit_status(
        on_event,
        format!(
            "Found {} log source files in the last 30 minute session.",
            recent_logs.len()
        ),
    );

    let documents = collect_log_documents(recent_logs, summary, on_event);
    if documents.is_empty() {
        push_warning(
            summary,
            on_event,
            StepKind::Logs,
            "No readable logs remained after scanning the selected folder.".to_owned(),
        );
        return;
    }

    summary.logs_processed = documents.len();

    match write_log_documents(
        &documents,
        &config.output_dir,
        config.max_words_per_chunk,
        config.max_chunks,
        |from, to| {
            emit_status(
                on_event,
                format!("Chunk {from} filled. Starting chunk {to}..."),
            );
        },
    ) {
        Ok(outcome) => {
            summary.log_chunks_generated = outcome.paths.len();
            summary.output_files.extend(outcome.paths);
            if outcome.truncated {
                let suffix = match config.max_chunks {
                    Some(limit) => format!(" of {limit}"),
                    None => String::new(),
                };
                push_warning(
                    summary,
                    on_event,
                    StepKind::Logs,
                    format!(
                        "Reached the maximum chunk count{suffix}. Remaining log content was omitted."
                    ),
                );
            }
            emit_status(
                on_event,
                format!(
                    "Logs processing complete. Total chunks generated: {}",
                    summary.log_chunks_generated
                ),
            );
        }
        Err(error) => push_warning(
            summary,
            on_event,
            StepKind::Logs,
            format!("Failed to write log output files: {error}"),
        ),
    }
}

fn process_mo2<F>(config: &RunConfig, summary: &mut RunSummary, on_event: &mut F)
where
    F: FnMut(ProgressEvent),
{
    emit_status(
        on_event,
        "--- Step 2: MO2 Profile Collection ---".to_owned(),
    );

    let Some(mo2_dir) = normalize_optional_dir(&config.mo2_dir) else {
        push_warning(
            summary,
            on_event,
            StepKind::Mo2,
            "No valid MO2 directory selected. Skipping MO2 files.".to_owned(),
        );
        return;
    };

    emit_status(
        on_event,
        format!("Selected MO2 folder: {}", mo2_dir.display()),
    );

    let output_path = config.output_dir.join(MO2_FILENAME);
    match write_mo2_output(&mo2_dir, &output_path, summary, on_event) {
        Ok(processed_count) => {
            summary.mo2_files_processed = processed_count;
            summary.output_files.push(output_path);
            emit_status(
                on_event,
                format!("MO2 Profile data saved to '{}'.", MO2_FILENAME),
            );
        }
        Err(error) => push_warning(
            summary,
            on_event,
            StepKind::Mo2,
            format!("Failed to write MO2 output file: {error}"),
        ),
    }
}

fn collect_log_candidates<F>(
    log_dir: &Path,
    allowed_extensions: &[&str],
    step: StepKind,
    summary: &mut RunSummary,
    on_event: &mut F,
) -> Option<Vec<LogCandidate>>
where
    F: FnMut(ProgressEvent),
{
    let read_dir = match fs::read_dir(log_dir) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            push_warning(
                summary,
                on_event,
                step,
                format!("Accessing directory failed: {error}"),
            );
            return None;
        }
    };

    let mut candidates = Vec::new();

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_warning(
                    summary,
                    on_event,
                    step,
                    format!("Skipping an unreadable log entry: {error}"),
                );
                continue;
            }
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };

        if !matches_ignore_ascii_case(extension, allowed_extensions) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                push_warning(
                    summary,
                    on_event,
                    step,
                    format!(
                        "Skipping '{}': failed to read metadata: {error}",
                        path.display()
                    ),
                );
                continue;
            }
        };

        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(error) => {
                push_warning(
                    summary,
                    on_event,
                    step,
                    format!(
                        "Skipping '{}': failed to read timestamp: {error}",
                        path.display()
                    ),
                );
                continue;
            }
        };

        candidates.push(LogCandidate {
            path: path.clone(),
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned(),
            modified,
        });
    }

    Some(candidates)
}

fn collect_log_documents<F>(
    candidates: Vec<LogCandidate>,
    summary: &mut RunSummary,
    on_event: &mut F,
) -> Vec<LogDocument>
where
    F: FnMut(ProgressEvent),
{
    let mut documents = Vec::new();

    for candidate in candidates {
        match fs::read(&candidate.path) {
            Ok(bytes) => {
                let content = normalize_for_windows_text_write(&decode_bytes_for_output(&bytes));
                documents.push(LogDocument {
                    name: candidate.name,
                    modified: candidate.modified,
                    content,
                });
            }
            Err(error) => push_warning(
                summary,
                on_event,
                StepKind::Logs,
                format!("Could not read {}: {error}", candidate.name),
            ),
        }
    }

    documents
}

fn write_log_documents<F>(
    documents: &[LogDocument],
    output_dir: &Path,
    max_words_per_chunk: usize,
    max_chunks: Option<usize>,
    mut on_rollover: F,
) -> io::Result<LogWriteOutcome>
where
    F: FnMut(usize, usize),
{
    if documents.is_empty() {
        return Ok(LogWriteOutcome {
            paths: Vec::new(),
            truncated: false,
        });
    }

    let mut writer = ChunkWriter::new(output_dir)?;
    let mut truncated = false;

    for document in documents {
        let header = format!(
            "{WINDOWS_NEWLINE}--- Log File: {} [{}] ---{WINDOWS_NEWLINE}",
            document.name,
            format_python_local_timestamp(document.modified)
        );
        let header_words = count_words(&header);

        if header_words <= max_words_per_chunk {
            if writer.current_word_count + header_words > max_words_per_chunk {
                let from = writer.current_chunk_index;
                let Some(to) = writer.rollover(output_dir, max_chunks)? else {
                    truncated = true;
                    break;
                };
                on_rollover(from, to);
            }

            writer.write_text(&header, header_words)?;
        } else if write_segmented_text(
            &header,
            &mut writer,
            output_dir,
            max_words_per_chunk,
            max_chunks,
            &mut on_rollover,
        )? {
            truncated = true;
            break;
        }

        for line in split_preserving_newlines(&document.content) {
            if write_segmented_text(
                line,
                &mut writer,
                output_dir,
                max_words_per_chunk,
                max_chunks,
                &mut on_rollover,
            )? {
                truncated = true;
                break;
            }
        }

        if truncated {
            break;
        }
    }

    Ok(LogWriteOutcome {
        paths: writer.finish()?,
        truncated,
    })
}

fn write_mo2_output<F>(
    mo2_dir: &Path,
    output_path: &Path,
    summary: &mut RunSummary,
    on_event: &mut F,
) -> io::Result<usize>
where
    F: FnMut(ProgressEvent),
{
    let read_dir = match fs::read_dir(mo2_dir) {
        Ok(read_dir) => Some(read_dir),
        Err(error) => {
            push_warning(
                summary,
                on_event,
                StepKind::Mo2,
                format!("Accessing profile directory failed: {error}"),
            );
            None
        }
    };

    let mut writer = BufWriter::new(File::create(output_path)?);
    let mut processed_count = 0usize;

    if let Some(read_dir) = read_dir {
        let mut found_any = false;

        for entry in read_dir {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    push_warning(
                        summary,
                        on_event,
                        StepKind::Mo2,
                        format!("Skipping an unreadable profile entry: {error}"),
                    );
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
                continue;
            };

            if !matches_ignore_ascii_case(extension, &["txt", "ini"]) {
                continue;
            }

            found_any = true;
            let name = path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or_default()
                .to_owned();

            writer
                .write_all(format!("--- Profile File: {name} ---{WINDOWS_NEWLINE}").as_bytes())?;

            match fs::read(&path) {
                Ok(bytes) => {
                    let content =
                        normalize_for_windows_text_write(&decode_bytes_for_output(&bytes));
                    writer.write_all(content.as_bytes())?;
                    writer.write_all(format!("{WINDOWS_NEWLINE}{WINDOWS_NEWLINE}").as_bytes())?;
                    processed_count += 1;
                }
                Err(error) => {
                    writer.write_all(
                        format!("[Error reading file: {error}]{WINDOWS_NEWLINE}").as_bytes(),
                    )?;
                }
            }
        }

        if !found_any {
            writer.write_all(b"No text or ini files found in selected folder.")?;
        }
    } else {
        writer.write_all(b"No text or ini files found in selected folder.")?;
    }

    writer.flush()?;
    Ok(processed_count)
}

fn normalize_optional_dir(path: &Option<PathBuf>) -> Option<PathBuf> {
    let path = path.as_ref()?;
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }

    Some(path.clone())
}

fn is_non_empty_path(path: &Option<PathBuf>) -> bool {
    path.as_ref()
        .is_some_and(|path| !path.as_os_str().is_empty())
}

fn normalize_for_windows_text_write(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    unified.replace('\n', WINDOWS_NEWLINE)
}

fn decode_bytes_for_output(bytes: &[u8]) -> String {
    sanitize_output_text(&String::from_utf8_lossy(bytes))
}

fn sanitize_output_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '\0' => {}
            '\r' | '\n' | '\t' => output.push(character),
            _ if character.is_control() => output.push(' '),
            _ => output.push(character),
        }
    }

    output
}

fn split_preserving_newlines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0usize;

    while let Some(relative_end) = text[start..].find(WINDOWS_NEWLINE) {
        let end = start + relative_end + WINDOWS_NEWLINE.len();
        lines.push(&text[start..end]);
        start = end;
    }

    if start < text.len() {
        lines.push(&text[start..]);
    }

    lines
}

fn write_segmented_text<F>(
    text: &str,
    writer: &mut ChunkWriter,
    output_dir: &Path,
    max_words_per_chunk: usize,
    max_chunks: Option<usize>,
    on_rollover: &mut F,
) -> io::Result<bool>
where
    F: FnMut(usize, usize),
{
    let mut remaining = text;

    while !remaining.is_empty() {
        let remaining_budget = max_words_per_chunk.saturating_sub(writer.current_word_count);
        if remaining_budget == 0 {
            let from = writer.current_chunk_index;
            let Some(to) = writer.rollover(output_dir, max_chunks)? else {
                return Ok(true);
            };
            on_rollover(from, to);
            continue;
        }

        let (end_index, words_in_segment) =
            take_prefix_within_word_limit(remaining, remaining_budget);

        if end_index == 0 {
            let from = writer.current_chunk_index;
            let Some(to) = writer.rollover(output_dir, max_chunks)? else {
                return Ok(true);
            };
            on_rollover(from, to);
            continue;
        }

        let segment = &remaining[..end_index];
        writer.write_text(segment, words_in_segment)?;
        remaining = &remaining[end_index..];
    }

    Ok(false)
}

fn take_prefix_within_word_limit(text: &str, word_limit: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }

    let mut words = 0usize;
    let mut in_word = false;
    let mut last_safe_end = 0usize;

    for (index, character) in text.char_indices() {
        if !is_word_char(character) {
            in_word = false;
            last_safe_end = index + character.len_utf8();
            continue;
        }

        if !in_word {
            if words == word_limit {
                break;
            }
            words += 1;
            in_word = true;
        }

        last_safe_end = index + character.len_utf8();
    }

    (last_safe_end, words)
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn matches_ignore_ascii_case(candidate: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| candidate.eq_ignore_ascii_case(option))
}

fn emit_status<F>(on_event: &mut F, message: String)
where
    F: FnMut(ProgressEvent),
{
    on_event(ProgressEvent::Status(message));
}

fn push_warning<F>(summary: &mut RunSummary, on_event: &mut F, step: StepKind, message: String)
where
    F: FnMut(ProgressEvent),
{
    let warning = CollectorWarning { step, message };
    summary.warnings.push(warning.clone());
    on_event(ProgressEvent::Warning(warning));
}

struct ChunkWriter {
    current_chunk_index: usize,
    current_word_count: usize,
    writer: BufWriter<File>,
    paths: Vec<PathBuf>,
}

impl ChunkWriter {
    fn new(output_dir: &Path) -> io::Result<Self> {
        let path = output_dir.join(log_output_filename(1));
        let writer = BufWriter::new(File::create(&path)?);

        Ok(Self {
            current_chunk_index: 1,
            current_word_count: 0,
            writer,
            paths: vec![path],
        })
    }

    fn rollover(
        &mut self,
        output_dir: &Path,
        max_chunks: Option<usize>,
    ) -> io::Result<Option<usize>> {
        if max_chunks.is_some_and(|max_chunks| self.current_chunk_index >= max_chunks) {
            self.writer.flush()?;
            return Ok(None);
        }

        self.writer.flush()?;
        self.current_chunk_index += 1;
        self.current_word_count = 0;

        let path = output_dir.join(log_output_filename(self.current_chunk_index));
        self.writer = BufWriter::new(File::create(&path)?);
        self.paths.push(path);

        Ok(Some(self.current_chunk_index))
    }

    fn write_text(&mut self, text: &str, words: usize) -> io::Result<()> {
        self.writer.write_all(text.as_bytes())?;
        self.current_word_count += words;
        Ok(())
    }

    fn finish(mut self) -> io::Result<Vec<PathBuf>> {
        self.writer.flush()?;
        Ok(self.paths)
    }
}

fn log_output_filename(index: usize) -> String {
    format!("{LOG_FILENAME_PREFIX}{index}{LOG_FILENAME_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone, Timelike};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn count_words_matches_python_split_behavior() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words(" one  two\tthree\nfour "), 4);
        assert_eq!(count_words("Skyrim"), 1);
        assert_eq!(
            count_words(r#"{"event":"boom","plugin":"skse","line":42}"#),
            6
        );
    }

    #[test]
    fn decode_bytes_for_output_strips_nuls_and_control_bytes() {
        let decoded = decode_bytes_for_output(b"C\0:\0\\\0G\0a\0m\0e\0s\0\x01\x02\r\n");
        assert_eq!(decoded, "C:\\Games  \r\n");
    }

    #[test]
    fn filter_recent_logs_uses_newest_file_minus_thirty_minutes() {
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(4_000);
        let within = base - Duration::from_secs(29 * 60);
        let outside = base - Duration::from_secs(31 * 60);

        let recent = filter_recent_logs(vec![
            LogCandidate {
                path: PathBuf::from("older.log"),
                name: "older.log".to_owned(),
                modified: outside,
            },
            LogCandidate {
                path: PathBuf::from("newest.log"),
                name: "newest.log".to_owned(),
                modified: base,
            },
            LogCandidate {
                path: PathBuf::from("within.log"),
                name: "within.log".to_owned(),
                modified: within,
            },
        ]);

        let names = recent
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["newest.log", "within.log"]);
    }

    #[test]
    fn default_output_dir_points_to_current_exe_directory() {
        let expected = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        assert_eq!(default_output_dir(), expected);
    }

    #[test]
    fn format_python_datetime_omits_fraction_when_zero() {
        let offset = FixedOffset::east_opt(3600).unwrap();
        let datetime = offset.with_ymd_and_hms(2025, 12, 14, 17, 22, 0).unwrap();
        assert_eq!(format_python_datetime(datetime), "2025-12-14 17:22:00");
    }

    #[test]
    fn format_python_datetime_includes_microseconds_when_present() {
        let offset = FixedOffset::east_opt(3600).unwrap();
        let mut datetime = offset.with_ymd_and_hms(2025, 12, 14, 17, 22, 0).unwrap();
        datetime = datetime.with_nanosecond(321_654_000).unwrap();

        assert_eq!(
            format_python_datetime(datetime),
            "2025-12-14 17:22:00.321654"
        );
    }

    #[test]
    fn write_log_documents_keeps_each_chunk_within_word_limit() {
        let output_dir = tempdir().unwrap();
        let documents = vec![LogDocument {
            name: "first.log".to_owned(),
            modified: SystemTime::UNIX_EPOCH,
            content: format!("alpha beta gamma delta epsilon zeta eta theta{WINDOWS_NEWLINE}"),
        }];

        let outcome =
            write_log_documents(&documents, output_dir.path(), 10, None, |_, _| {}).unwrap();

        assert!(!outcome.truncated);
        assert_eq!(outcome.paths.len(), 2);

        for path in outcome.paths {
            let chunk = fs::read_to_string(path).unwrap();
            assert!(count_words(&chunk) <= 10);
        }
    }

    #[test]
    fn write_log_documents_splits_long_line_across_chunks() {
        let output_dir = tempdir().unwrap();
        let documents = vec![LogDocument {
            name: "first.log".to_owned(),
            modified: SystemTime::UNIX_EPOCH,
            content: format!(
                "one two three four five six seven eight nine ten eleven twelve{WINDOWS_NEWLINE}"
            ),
        }];

        let outcome =
            write_log_documents(&documents, output_dir.path(), 10, None, |_, _| {}).unwrap();

        assert_eq!(outcome.paths.len(), 3);

        let first_chunk =
            fs::read_to_string(output_dir.path().join(log_output_filename(1))).unwrap();
        let second_chunk =
            fs::read_to_string(output_dir.path().join(log_output_filename(2))).unwrap();
        let third_chunk =
            fs::read_to_string(output_dir.path().join(log_output_filename(3))).unwrap();

        assert!(count_words(&first_chunk) <= 10);
        assert!(count_words(&second_chunk) <= 10);
        assert!(count_words(&third_chunk) <= 10);
        assert!(first_chunk.contains("--- Log File: first.log"));
        assert!(second_chunk.contains("one two three four five six seven eight nine ten"));
        assert!(third_chunk.contains("eleven twelve"));
    }

    #[test]
    fn write_log_documents_truncates_when_chunk_limit_is_reached() {
        let output_dir = tempdir().unwrap();
        let documents = vec![LogDocument {
            name: "first.log".to_owned(),
            modified: SystemTime::UNIX_EPOCH,
            content: format!("one two three four five six seven eight nine ten{WINDOWS_NEWLINE}"),
        }];

        let outcome =
            write_log_documents(&documents, output_dir.path(), 6, Some(2), |_, _| {}).unwrap();

        assert!(outcome.truncated);
        assert_eq!(outcome.paths.len(), 2);

        for path in outcome.paths {
            let chunk = fs::read_to_string(path).unwrap();
            assert!(count_words(&chunk) <= 6);
        }
    }
}
