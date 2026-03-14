# CrashLogCollector

CrashLogCollector is a Windows-first desktop app for Skyrim SE/AE that gathers your recent crash context and packages it into AI-friendly text files. The current app is written in Rust with an `egui` GUI and is meant to replace the old batch/Python launcher workflow.

## What it does

- Collects the most recent top-level SKSE logs from the last 30 minutes of activity.
- Optionally also pulls files from `MO2\overwrite\SKSE\Plugins`.
- Bundles your Mod Organizer 2 profile text and ini files into `MO2 PROFILE.txt`.
- Splits generated `LOGS_part_N.txt` files by a configurable maximum word count.
- Optionally caps the number of log chunks created.
- Remembers the last folders you used.
- Writes generated output next to the executable so the output location is predictable.

## Supported overwrite-plugin file types

When the optional overwrite plugins folder is set, these file types are accepted:

- `.txt`
- `.log`
- `.json`
- `.jsonl`
- `.dmp`

Binary-ish dump content is sanitized before being written so the generated text files stay upload-friendly for tools like NotebookLM.

## How to use

1. Download the latest Windows release.
2. Put the executable wherever you want the generated files to appear.
3. Launch `crashlog-collector.exe`.
4. Set your Skyrim logs folder, optional overwrite plugins folder, and MO2 profile folder.
5. Adjust the maximum words per chunk and optional max chunks if needed.
6. Click `Run`.

The app creates:

- `LOGS_part_N.txt`
- `MO2 PROFILE.txt`

## Building from source

```powershell
cargo build --release
```

The release binary will be written to `target\release\crashlog-collector.exe`.

## Repository layout

- `src/` contains the Rust GUI app and collector logic.
- `tests/` contains collector regression tests and fixtures.
- `1.0/`, `1.1/`, and `2.0/` preserve older script-based versions.

## License

This repository is licensed under the GPL-3.0 license. See `LICENSE` for details.
