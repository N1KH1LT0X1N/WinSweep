//! Scan view

use crate::viewmodel::scan::{categorize_path, ScanResult, SortColumn};
use crate::viewmodel::WinSweepViewModel;
use crate::views::utils::format_bytes;
use eframe::egui;
use egui_extras::{Column, TableBuilder};

pub fn show_scan(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("System Scan");
    ui.separator();

    // Scan options
    ui.horizontal(|ui| {
        ui.label("Scan Location:");
        ui.text_edit_singleline(&mut viewmodel.scan.scan_options.path);

        if ui.button("Browse...").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                viewmodel.scan.scan_options.path = folder.display().to_string();
            }
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
                let path = viewmodel.scan.scan_options.path.clone();
                if let Some(rt) = viewmodel.runtime {
                    viewmodel.scan.start_scan(&path, rt);
                }
            }
        }

        if !viewmodel.scan.scan_results.is_empty() {
            let n_selected = viewmodel.scan.selected_rows.len();

            ui.separator();
            if ui.button("☑ Select All").clicked() {
                let len = viewmodel.scan.scan_results.len();
                viewmodel.scan.selected_rows = (0..len).collect();
            }
            if ui.button("☐ Deselect All").clicked() {
                viewmodel.scan.selected_rows.clear();
            }

            ui.separator();
            if ui
                .add_enabled(
                    n_selected > 0,
                    egui::Button::new(format!("🗑️ Delete Selected ({})", n_selected)),
                )
                .clicked()
            {
                trigger_delete_selected(viewmodel);
            }

            if ui.button("🗑️ Delete All").clicked() {
                trigger_delete_all(viewmodel);
            }

            if ui.button("📤 Export CSV").clicked() {
                export_csv(viewmodel);
            }

            // Total selected size
            if n_selected > 0 {
                let sel_size: u64 = viewmodel
                    .scan
                    .selected_rows
                    .iter()
                    .filter_map(|&i| viewmodel.scan.scan_results.get(i))
                    .map(|r| r.size)
                    .sum();
                ui.label(format!("Selected: {}", format_bytes(sel_size)));
            }
        }
    });

    // Category breakdown
    if let Some(ref breakdown) = viewmodel.scan.pending_category_breakdown {
        ui.separator();
        ui.heading("Breakdown by Category");

        let results = &viewmodel.scan.scan_results;
        let artifacts: Vec<&ScanResult> = results
            .iter()
            .filter(|r| categorize_path(&r.path) == "Artifacts")
            .collect();
        let temp: Vec<&ScanResult> = results
            .iter()
            .filter(|r| categorize_path(&r.path) == "Temp")
            .collect();
        let package_cache: Vec<&ScanResult> = results
            .iter()
            .filter(|r| categorize_path(&r.path) == "Package Cache")
            .collect();
        let recycle_bin: Vec<&ScanResult> = results
            .iter()
            .filter(|r| categorize_path(&r.path) == "Recycle Bin")
            .collect();
        let other: Vec<&ScanResult> = results
            .iter()
            .filter(|r| categorize_path(&r.path) == "Other")
            .collect();

        show_category_section(
            ui,
            "📦 Artifacts",
            breakdown.artifact_bytes,
            egui::Color32::from_rgb(180, 140, 80),
            &artifacts,
        );
        show_category_section(
            ui,
            "🌡️ Temp Files",
            breakdown.temp_bytes,
            egui::Color32::from_rgb(200, 100, 80),
            &temp,
        );
        show_category_section(
            ui,
            "📦 Package Cache",
            breakdown.package_cache_bytes,
            egui::Color32::from_rgb(80, 140, 180),
            &package_cache,
        );
        show_category_section(
            ui,
            "♻️ Recycle Bin",
            breakdown.recycle_bin_bytes,
            egui::Color32::from_rgb(100, 200, 100),
            &recycle_bin,
        );
        show_category_section(
            ui,
            "📁 Other",
            breakdown.other_bytes,
            egui::Color32::GRAY,
            &other,
        );
    }

    // Scan results table
    if !viewmodel.scan.scan_results.is_empty() {
        ui.separator();
        ui.heading("Scan Results");
        ui.label(format!("{} items found", viewmodel.scan.scan_results.len()));

        show_results_table(ui, viewmodel);
    }
}

fn show_category_section(
    ui: &mut egui::Ui,
    label: &str,
    bytes: u64,
    _color: egui::Color32,
    items: &[&ScanResult],
) {
    egui::CollapsingHeader::new(format!("{} — {}", label, format_bytes(bytes)))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for r in items {
                        ui.horizontal(|ui| {
                            ui.label(&r.path);
                            ui.label(format_bytes(r.size));
                        });
                    }
                });
        });
}

fn show_results_table(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    // Clone data to avoid borrow issues inside closures
    let results: Vec<_> = viewmodel
        .scan
        .scan_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                i,
                r.path.clone(),
                r.size,
                r.file_count,
                r.directory_count,
                r.last_modified.clone(),
                r.file_type.clone(),
            )
        })
        .collect();
    let sort_col = viewmodel.scan.sort_column;
    let sort_desc = viewmodel.scan.sort_descending;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(30.0))
        .column(Column::remainder().at_least(250.0))
        .column(Column::auto().at_least(80.0))
        .column(Column::auto().at_least(60.0))
        .column(Column::auto().at_least(60.0))
        .column(Column::auto().at_least(130.0))
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.label("");
            });
            header.col(|ui| {
                let label = if sort_col == SortColumn::Path {
                    format!("Path {}", if sort_desc { "▼" } else { "▲" })
                } else {
                    "Path".to_string()
                };
                if ui.button(label).clicked() {
                    viewmodel.scan.toggle_sort(SortColumn::Path);
                }
            });
            header.col(|ui| {
                let label = if sort_col == SortColumn::Size {
                    format!("Size {}", if sort_desc { "▼" } else { "▲" })
                } else {
                    "Size".to_string()
                };
                if ui.button(label).clicked() {
                    viewmodel.scan.toggle_sort(SortColumn::Size);
                }
            });
            header.col(|ui| {
                let label = if sort_col == SortColumn::FileCount {
                    format!("Files {}", if sort_desc { "▼" } else { "▲" })
                } else {
                    "Files".to_string()
                };
                if ui.button(label).clicked() {
                    viewmodel.scan.toggle_sort(SortColumn::FileCount);
                }
            });
            header.col(|ui| {
                ui.label("Dirs");
            });
            header.col(|ui| {
                let label = if sort_col == SortColumn::LastModified {
                    format!("Modified {}", if sort_desc { "▼" } else { "▲" })
                } else {
                    "Modified".to_string()
                };
                if ui.button(label).clicked() {
                    viewmodel.scan.toggle_sort(SortColumn::LastModified);
                }
            });
        })
        .body(|body| {
            body.rows(18.0, results.len(), |mut row| {
                let (i, path, size, file_count, dir_count, last_modified, _file_type) =
                    &results[row.index()];
                row.col(|ui| {
                    let mut checked = viewmodel.scan.selected_rows.contains(i);
                    if ui.checkbox(&mut checked, "").changed() {
                        if checked {
                            viewmodel.scan.selected_rows.insert(*i);
                        } else {
                            viewmodel.scan.selected_rows.remove(i);
                        }
                    }
                });
                row.col(|ui| {
                    ui.label(path);
                });
                row.col(|ui| {
                    ui.label(format_bytes(*size));
                });
                row.col(|ui| {
                    ui.label(file_count.to_string());
                });
                row.col(|ui| {
                    ui.label(dir_count.to_string());
                });
                row.col(|ui| {
                    ui.label(last_modified);
                });
            });
        });
}

fn trigger_delete_selected(viewmodel: &mut WinSweepViewModel) {
    if viewmodel.scan.selected_rows.is_empty() {
        return;
    }
    let mut indices: Vec<usize> = viewmodel.scan.selected_rows.iter().copied().collect();
    indices.sort_by(|a, b| b.cmp(a)); // descending so removal doesn't shift indices
    let mut items = Vec::new();
    for i in &indices {
        if *i < viewmodel.scan.raw_results.len() {
            items.push(viewmodel.scan.raw_results[*i].clone());
        }
    }
    // Remove from view lists (reverse order to preserve indices)
    for i in indices {
        if i < viewmodel.scan.raw_results.len() {
            viewmodel.scan.raw_results.remove(i);
        }
        if i < viewmodel.scan.scan_results.len() {
            viewmodel.scan.scan_results.remove(i);
        }
    }
    viewmodel.scan.selected_rows.clear();
    if !items.is_empty() {
        let desc = "Delete selected scan items".to_string();
        if viewmodel.config().cleanup.cleanup_confirm_delete {
            viewmodel.set_pending_cleanup(items, desc);
        } else {
            viewmodel.start_cleanup_task(items, desc);
        }
    }
}

fn trigger_delete_all(viewmodel: &mut WinSweepViewModel) {
    let items = std::mem::take(&mut viewmodel.scan.raw_results);
    viewmodel.scan.delete_all();
    if !items.is_empty() {
        let desc = "Delete all scan items".to_string();
        if viewmodel.config().cleanup.cleanup_confirm_delete {
            viewmodel.set_pending_cleanup(items, desc);
        } else {
            viewmodel.start_cleanup_task(items, desc);
        }
    }
}

fn export_csv(viewmodel: &mut WinSweepViewModel) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name("winsweep_scan_results.csv")
        .save_file()
    {
        let mut lines = vec!["path,size_bytes,files,directories,modified,category".to_string()];
        for r in &viewmodel.scan.scan_results {
            let cat = categorize_path(&r.path);
            lines.push(format!(
                "\"{}\",{},{},{},\"{}\",\"{}\"",
                r.path.replace('"', "\"\""),
                r.size,
                r.file_count,
                r.directory_count,
                r.last_modified,
                cat
            ));
        }
        let csv = lines.join("\n");
        if let Err(e) = std::fs::write(&path, csv) {
            tracing::warn!("Failed to write CSV: {}", e);
        }
    }
}
