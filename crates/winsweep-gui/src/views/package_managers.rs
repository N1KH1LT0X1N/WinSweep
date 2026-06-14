//! Package managers view

use crate::viewmodel::WinSweepViewModel;
use crate::views::utils::format_bytes;
use eframe::egui;

pub fn show_package_managers(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Package Managers");
    ui.separator();

    // Refresh button
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            viewmodel.start_package_manager_refresh_task();
        }

        ui.label("Manage package manager caches");
    });

    // Package managers list
    if !viewmodel.package_managers.managers.is_empty() {
        ui.separator();

        let managers: Vec<_> = viewmodel
            .package_managers
            .managers
            .iter()
            .enumerate()
            .map(|(i, m)| {
                (
                    i,
                    m.display_name.clone(),
                    m.installed,
                    m.version.clone(),
                    m.cache_size,
                    m.cache_paths.clone(),
                    viewmodel.package_managers.selected_manager == Some(i),
                )
            })
            .collect();

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, display_name, installed, version, cache_size, cache_paths, selected) in
                    managers
                {
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
                            let status_color = if installed {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            };

                            ui.colored_label(status_color, if installed { "✓" } else { "✗" });

                            // Manager info
                            ui.vertical(|ui| {
                                ui.label(&display_name);
                                ui.label(format!(
                                    "Version: {}",
                                    version.as_deref().unwrap_or("Not installed")
                                ));
                                ui.label(format!("Cache size: {}", format_bytes(cache_size)));
                            });

                            // Actions
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if installed && ui.button("🗑️ Clean Cache").clicked() {
                                        viewmodel.start_package_manager_clean_task(i);
                                    }

                                    if ui.button("ℹ️ Info").clicked() {
                                        viewmodel.package_managers.selected_manager = Some(i);
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
                    if selected && !cache_paths.is_empty() {
                        ui.indent(1, |ui| {
                            ui.label("Cache paths:");
                            for path in &cache_paths {
                                ui.label(format!("  • {}", path));
                            }
                        });
                    }
                }
            });

        // Cleanup actions
        ui.separator();
        ui.horizontal(|ui| {
            if viewmodel.is_operation_running() {
                ui.spinner();
                ui.add(egui::ProgressBar::new(
                    viewmodel.operation_progress().unwrap_or(0.0),
                ));
            } else {
                if ui.button("🗑️ Clean Selected").clicked() {
                    if let Some(i) = viewmodel.package_managers.selected_manager {
                        viewmodel.start_package_manager_clean_task(i);
                    }
                }

                if ui.button("🗑️ Clean All").clicked() {
                    viewmodel.start_package_manager_clean_all_task();
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
