//! Scan view

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

pub fn show_scan(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("System Scan");
    ui.separator();

    // Scan options
    ui.horizontal(|ui| {
        ui.label("Scan Location:");
        ui.text_edit_singleline(&mut viewmodel.scan.scan_options.path);

        if ui.button("Browse...").clicked() {
            // TODO: Open file dialog
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut viewmodel.scan.scan_options.include_hidden,
            "Include hidden files",
        );
        ui.checkbox(
            &mut viewmodel.scan.scan_options.include_system,
            "Include system files",
        );

        ui.label("Min file size:");
        ui.add(
            egui::DragValue::new(&mut viewmodel.scan.scan_options.min_file_size).suffix(" bytes"),
        );
    });

    // Scan controls
    ui.separator();
    ui.horizontal(|ui| {
        if viewmodel.scan.scan_in_progress {
            if ui.button("⏹ Stop").clicked() {
                viewmodel.scan.stop_scan();
            }

            ui.add(
                egui::ProgressBar::new(viewmodel.scan.scan_progress).text(format!(
                    "Scanning... {:.1}%",
                    viewmodel.scan.scan_progress * 100.0
                )),
            );
        } else {
            if ui.button("🔍 Start Scan").clicked() {
                viewmodel.scan.start_scan(&viewmodel.scan.scan_options.path);
            }
        }

        if !viewmodel.scan.scan_results.is_empty() {
            if ui.button("🗑️ Delete Selected").clicked() {
                viewmodel.scan.delete_selected();
            }

            if ui.button("🗑️ Delete All").clicked() {
                // TODO: Delete all results
            }
        }
    });

    // Scan results
    if !viewmodel.scan.scan_results.is_empty() {
        ui.separator();
        ui.heading("Scan Results");

        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label("Path");
                    ui.label("Size");
                    ui.label("Files");
                    ui.label("Modified");
                });
                ui.separator();

                // Results
                for (i, result) in viewmodel.scan.scan_results.iter().enumerate() {
                    let selected = viewmodel.scan.selected_result == Some(i);

                    if ui.selectable_label(selected, &result.path).clicked() {
                        viewmodel.scan.selected_result = Some(i);
                    }

                    if selected {
                        ui.horizontal(|ui| {
                            ui.label(format!("Size: {}", format_bytes(result.size)));
                            ui.label(format!("Files: {}", result.file_count));
                            ui.label(format!("Dirs: {}", result.directory_count));
                            ui.label(&result.last_modified);
                        });
                    }
                }
            });
    }
}

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
