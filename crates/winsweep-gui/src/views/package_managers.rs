//! Package managers view

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

pub fn show_package_managers(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Package Managers");
    ui.separator();

    // Refresh button
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            // TODO: Refresh package managers
        }

        ui.label("Manage package manager caches");
    });

    // Package managers list
    if !viewmodel.package_managers.managers.is_empty() {
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, manager) in viewmodel.package_managers.managers.iter().enumerate() {
                    let selected = viewmodel.package_managers.selected_manager == Some(i);
                    let frame = egui::Frame::none()
                        .fill(if selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            ui.visuals().window_fill()
                        })
                        .stroke(if selected {
                            ui.visuals().selection.stroke
                        } else {
                            egui::Stroke::NONE
                        });

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Status indicator
                            let status_color = if manager.installed {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            };

                            ui.colored_label(
                                status_color,
                                if manager.installed { "✓" } else { "✗" },
                            );

                            // Manager info
                            ui.vertical(|ui| {
                                ui.label(&manager.display_name);
                                ui.label(format!(
                                    "Version: {}",
                                    manager.version.as_deref().unwrap_or("Not installed")
                                ));
                                ui.label(format!(
                                    "Cache size: {}",
                                    format_bytes(manager.cache_size)
                                ));
                            });

                            // Actions
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if manager.installed {
                                        if ui.button("🗑️ Clean Cache").clicked() {
                                            // TODO: Clean cache
                                        }
                                    }

                                    if ui.button("ℹ️ Info").clicked() {
                                        // TODO: Show cache info
                                    }
                                },
                            );
                        });

                        if ui
                            .allocate_response(egui::Vec2::ZERO, egui::Sense::click())
                            .clicked()
                        {
                            viewmodel.package_managers.selected_manager = Some(i);
                        }
                    });

                    // Show cache paths if selected
                    if selected && !manager.cache_paths.is_empty() {
                        ui.indent(1, |ui| {
                            ui.label("Cache paths:");
                            for path in &manager.cache_paths {
                                ui.label(format!("  • {}", path));
                            }
                        });
                    }
                }
            });

        // Cleanup actions
        ui.separator();
        ui.horizontal(|ui| {
            if viewmodel.package_managers.operation_in_progress {
                ui.spinner();
                ui.add(egui::ProgressBar::new(
                    viewmodel.package_managers.operation_progress,
                ));
            } else {
                if ui.button("🗑️ Clean Selected").clicked() {
                    // TODO: Clean selected manager
                }

                if ui.button("🗑️ Clean All").clicked() {
                    // TODO: Clean all managers
                }
            }
        });
    } else {
        ui.label("No package managers found. Install package managers to use this feature.");
    }

    // Status message
    if let Some(ref msg) = viewmodel.package_managers.status_message {
        ui.separator();
        ui.label(msg);
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
