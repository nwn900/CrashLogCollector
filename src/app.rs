use crate::collector::{
    CollectorWarning, DEFAULT_MAX_WORDS_PER_CHUNK, ProgressEvent, RunConfig, RunSummary,
    default_output_dir, default_skyrim_logs_dir, spawn_collection_thread,
};
use crate::settings::{PersistedSettings, load_settings, save_settings};
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

pub struct CrashLogCollectorApp {
    log_dir_input: String,
    overwrite_plugins_dir_input: String,
    mo2_dir_input: String,
    output_dir_input: String,
    max_words_input: String,
    max_chunks_input: String,
    status_lines: Vec<String>,
    warnings: Vec<CollectorWarning>,
    summary: Option<RunSummary>,
    is_running: bool,
    progress_receiver: Option<Receiver<ProgressEvent>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl CrashLogCollectorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_visuals(&cc.egui_ctx);

        let mut app = Self {
            log_dir_input: default_skyrim_logs_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            overwrite_plugins_dir_input: String::new(),
            mo2_dir_input: String::new(),
            output_dir_input: default_output_dir().display().to_string(),
            max_words_input: DEFAULT_MAX_WORDS_PER_CHUNK.to_string(),
            max_chunks_input: String::new(),
            status_lines: vec![
                "Ready to collect Skyrim crash logs, overwrite plugin logs, and MO2 profile files."
                    .to_owned(),
            ],
            warnings: Vec::new(),
            summary: None,
            is_running: false,
            progress_receiver: None,
            worker_handle: None,
        };

        match load_settings() {
            Ok(Some(settings)) => app.apply_persisted_settings(settings),
            Ok(None) => {}
            Err(error) => app
                .status_lines
                .push(format!("Could not load remembered folder paths: {error}")),
        }

        app
    }

    fn start_run(&mut self) {
        self.status_lines.clear();
        self.warnings.clear();
        self.summary = None;

        let max_words_per_chunk = match self.parse_max_words() {
            Ok(value) => value,
            Err(message) => {
                self.status_lines.push(message);
                return;
            }
        };
        let max_chunks = match self.parse_max_chunks() {
            Ok(value) => value,
            Err(message) => {
                self.status_lines.push(message);
                return;
            }
        };

        let output_dir = default_output_dir();

        self.output_dir_input = output_dir.display().to_string();

        let config = RunConfig {
            log_dir: to_optional_path(&self.log_dir_input),
            overwrite_plugins_dir: to_optional_path(&self.overwrite_plugins_dir_input),
            mo2_dir: to_optional_path(&self.mo2_dir_input),
            output_dir,
            max_words_per_chunk,
            max_chunks,
        };

        self.save_current_paths();

        let (sender, receiver) = mpsc::channel();
        self.progress_receiver = Some(receiver);
        self.worker_handle = Some(spawn_collection_thread(config, sender));
        self.is_running = true;
        self.status_lines.push("Collector started...".to_owned());
    }

    fn parse_max_words(&self) -> Result<usize, String> {
        let trimmed = self.max_words_input.trim();
        if trimmed.is_empty() {
            return Err("Max words per chunk is required.".to_owned());
        }

        let value = trimmed
            .parse::<usize>()
            .map_err(|_| "Max words per chunk must be a positive integer.".to_owned())?;

        if value == 0 {
            return Err("Max words per chunk must be greater than zero.".to_owned());
        }

        Ok(value)
    }

    fn parse_max_chunks(&self) -> Result<Option<usize>, String> {
        let trimmed = self.max_chunks_input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let value = trimmed
            .parse::<usize>()
            .map_err(|_| "Max chunks must be a positive integer when provided.".to_owned())?;

        if value == 0 {
            return Err("Max chunks must be greater than zero when provided.".to_owned());
        }

        Ok(Some(value))
    }

    fn poll_progress(&mut self) {
        let mut finished_summary = None;
        let mut disconnected = false;

        if let Some(receiver) = &self.progress_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(event) => match event {
                        ProgressEvent::Status(message) => self.status_lines.push(message),
                        ProgressEvent::Warning(warning) => {
                            self.status_lines.push(format!(
                                "[{}] {}",
                                warning.step.label(),
                                warning.message
                            ));
                            self.warnings.push(warning);
                        }
                        ProgressEvent::Finished(summary) => {
                            finished_summary = Some(summary);
                            break;
                        }
                    },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if let Some(summary) = finished_summary {
            self.is_running = false;
            self.summary = Some(summary);
            self.progress_receiver = None;

            if let Some(handle) = self.worker_handle.take() {
                let _ = handle.join();
            }
        } else if disconnected {
            self.is_running = false;
            self.progress_receiver = None;
            self.status_lines
                .push("Collector thread ended unexpectedly.".to_owned());

            if let Some(handle) = self.worker_handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn apply_persisted_settings(&mut self, settings: PersistedSettings) {
        self.log_dir_input = settings.log_dir;
        self.overwrite_plugins_dir_input = settings.overwrite_plugins_dir;
        self.mo2_dir_input = settings.mo2_dir;
        self.output_dir_input = default_output_dir().display().to_string();
        self.max_chunks_input = settings.max_chunks;
    }

    fn current_settings(&self) -> PersistedSettings {
        PersistedSettings {
            log_dir: self.log_dir_input.trim().to_owned(),
            overwrite_plugins_dir: self.overwrite_plugins_dir_input.trim().to_owned(),
            mo2_dir: self.mo2_dir_input.trim().to_owned(),
            output_dir: default_output_dir().display().to_string(),
            max_chunks: self.max_chunks_input.trim().to_owned(),
        }
    }

    fn save_current_paths(&mut self) {
        if let Err(error) = save_settings(&self.current_settings()) {
            self.status_lines
                .push(format!("Could not save remembered folder paths: {error}"));
        }
    }
}

impl eframe::App for CrashLogCollectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_progress();

        if self.is_running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut should_save_paths = false;

            ui.vertical(|ui| {
                ui.heading(RichText::new("CrashLog Collector").size(28.0));
                ui.label(
                    "Collect the newest Skyrim crash-session logs, optional overwrite plugin logs, and package your MO2 profile files into shareable text outputs.",
                );
                ui.add_space(10.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(251, 247, 240))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Folders").strong());
                        ui.add_space(6.0);
                        should_save_paths |= folder_row(
                            ui,
                            "Skyrim logs",
                            &mut self.log_dir_input,
                            self.is_running,
                            "Select your SKSE Logs folder",
                        );
                        should_save_paths |= folder_row(
                            ui,
                            "Overwrite plugins",
                            &mut self.overwrite_plugins_dir_input,
                            self.is_running,
                            "Select your MO2 Overwrite\\SKSE\\Plugins folder",
                        );
                        should_save_paths |= folder_row(
                            ui,
                            "MO2 profile",
                            &mut self.mo2_dir_input,
                            self.is_running,
                            "Select your MO2 profile folder",
                        );
                    });

                ui.add_space(8.0);

                egui::CollapsingHeader::new("Settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Output folder");
                            let mut display_value = self.output_dir_input.clone();
                            ui.add_enabled(
                                false,
                                egui::TextEdit::singleline(&mut display_value)
                                    .desired_width(ui.available_width() - 110.0),
                            );
                        });
                        ui.label("Generated files are always saved next to the executable.");

                        ui.horizontal(|ui| {
                            ui.label("Max words per chunk");
                            ui.add_enabled(
                                !self.is_running,
                                egui::TextEdit::singleline(&mut self.max_words_input)
                                    .desired_width(120.0),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label("Max chunks");
                            ui.add_enabled(
                                !self.is_running,
                                egui::TextEdit::singleline(&mut self.max_chunks_input)
                                    .desired_width(120.0)
                                    .hint_text("Unlimited"),
                            );
                        });
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let run_label = if self.is_running {
                        "Collecting..."
                    } else {
                        "Run Collector"
                    };

                    if ui
                        .add_enabled(
                            !self.is_running,
                            egui::Button::new(RichText::new(run_label).strong())
                                .min_size(egui::vec2(140.0, 36.0)),
                        )
                        .clicked()
                    {
                        self.start_run();
                    }

                    if self.is_running {
                        ui.label("The worker runs in the background so the window stays responsive.");
                    } else {
                        ui.label("Leave the optional folders blank if you want to skip that source or step.");
                    }
                });

                ui.add_space(12.0);

                if let Some(summary) = &self.summary {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(239, 244, 236))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Last Run").strong());
                            ui.label(format!(
                                "Logs processed: {} | Chunks written: {} | MO2 files processed: {}",
                                summary.logs_processed,
                                summary.log_chunks_generated,
                                summary.mo2_files_processed
                            ));
                            ui.label(format!("Warnings: {}", summary.warnings.len()));

                            if !summary.output_files.is_empty() {
                                ui.add_space(4.0);
                                for path in &summary.output_files {
                                    ui.label(path.display().to_string());
                                }
                            }
                        });
                    ui.add_space(10.0);
                }

                if !self.warnings.is_empty() {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(255, 244, 230))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Warnings")
                                    .strong()
                                    .color(Color32::from_rgb(140, 82, 33)),
                            );
                            for warning in &self.warnings {
                                ui.label(format!("[{}] {}", warning.step.label(), warning.message));
                            }
                        });
                    ui.add_space(10.0);
                }

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(243, 239, 232))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Status").strong());
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .max_height(ui.available_height() - 8.0)
                            .show(ui, |ui| {
                                for line in &self.status_lines {
                                    ui.label(RichText::new(line).monospace());
                                }
                            });
                    });
            });

            if should_save_paths {
                self.save_current_paths();
            }
        });
    }
}

fn folder_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    is_running: bool,
    dialog_title: &str,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        let response = ui.add_enabled(
            !is_running,
            egui::TextEdit::singleline(value).desired_width(ui.available_width() - 110.0),
        );
        changed |= response.changed();

        if ui
            .add_enabled(!is_running, egui::Button::new("Browse..."))
            .clicked()
        {
            let dialog = if value.trim().is_empty() {
                rfd::FileDialog::new()
            } else {
                rfd::FileDialog::new().set_directory(value.trim())
            };

            if let Some(folder) = dialog.set_title(dialog_title).pick_folder() {
                *value = folder.display().to_string();
                changed = true;
            }
        }
    });

    changed
}

fn configure_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = Color32::from_rgb(247, 243, 236);
    visuals.panel_fill = Color32::from_rgb(247, 243, 236);
    visuals.widgets.active.bg_fill = Color32::from_rgb(41, 86, 108);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(78, 118, 136);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(225, 220, 211);
    visuals.hyperlink_color = Color32::from_rgb(41, 86, 108);
    visuals.selection.bg_fill = Color32::from_rgb(180, 205, 217);
    ctx.set_visuals(visuals);
}

fn to_optional_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

impl Drop for CrashLogCollectorApp {
    fn drop(&mut self) {
        let _ = save_settings(&self.current_settings());
    }
}
