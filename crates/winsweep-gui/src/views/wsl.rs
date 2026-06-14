//! WSL view

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

pub fn show_wsl(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("WSL Management");
    ui.separator();

    // Refresh button
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            if let Some(ref wsl_detector) = viewmodel.wsl_detector {
                viewmodel.wsl.refresh_distributions(wsl_detector);
            }
        }

        ui.label("Windows Subsystem for Linux distributions");
    });

    // WSL distributions list
    if !viewmodel.wsl.distributions.is_empty() {
        ui.separator();

        // Distribution list
        let distributions: Vec<_> = viewmodel
            .wsl
            .distributions
            .iter()
            .enumerate()
            .map(|(i, dist)| {
                (
                    i,
                    dist.name.clone(),
                    dist.version,
                    dist.state,
                    dist.size_gb,
                    dist.path.clone(),
                    viewmodel.wsl.selected_distribution == Some(i),
                )
            })
            .collect();

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, name, version, state, size_gb, path, selected) in distributions {
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
                            let status_color = match state {
                                winsweep_core::WslState::Running => egui::Color32::GREEN,
                                winsweep_core::WslState::Stopped => egui::Color32::RED,
                                _ => egui::Color32::YELLOW,
                            };

                            ui.colored_label(status_color, "●");

                            // Distribution info
                            ui.vertical(|ui| {
                                ui.label(format!(
                                    "{} (WSL{})",
                                    name,
                                    match version {
                                        winsweep_core::WslVersion::Wsl1 => "1",
                                        winsweep_core::WslVersion::Wsl2 => "2",
                                        winsweep_core::WslVersion::Unknown => "?",
                                    }
                                ));
                                ui.label(format!("State: {:?}", state));
                                ui.label(format!("Size: {:.1} GB", size_gb));
                                ui.label(format!("Path: {}", path));
                            });

                            // Actions
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let name = name.clone();
                                    if state == winsweep_core::WslState::Running {
                                        if ui.button("⏹ Stop").clicked() {
                                            viewmodel.wsl.stop_distribution(&name);
                                        }
                                    } else {
                                        if ui.button("▶ Start").clicked() {
                                            viewmodel.wsl.start_distribution(&name);
                                        }
                                    }

                                    if ui.button("🗑️ Unregister").clicked() {
                                        if viewmodel.wsl.selected_distribution == Some(i) {
                                            viewmodel.wsl.unregister_distribution(&name);
                                        } else {
                                            viewmodel.wsl.selected_distribution = Some(i);
                                        }
                                    }
                                },
                            );
                        });

                        if ui
                            .allocate_response(egui::Vec2::ZERO, egui::Sense::click())
                            .clicked()
                        {
                            viewmodel.wsl.selected_distribution = Some(i);
                        }
                    });
                }
            });

        // Selected distribution actions
        if let Some(index) = viewmodel.wsl.selected_distribution {
            if index < viewmodel.wsl.distributions.len() {
                ui.separator();

                let dist_name = viewmodel.wsl.distributions[index].name.clone();
                ui.horizontal(|ui| {
                    ui.heading(format!("{} Actions", dist_name));

                    if viewmodel.wsl.compact_in_progress {
                        ui.spinner();
                        ui.label("Compacting...");
                        ui.add(egui::ProgressBar::new(viewmodel.wsl.compact_progress));
                    } else {
                        if ui.button("🗜️ Compact Disk").clicked() {
                            let dist_name = viewmodel.wsl.distributions[index].name.clone();
                            viewmodel.wsl.start_compact();
                            viewmodel.start_wsl_compact_task(dist_name);
                        }

                        if ui.button("📁 Open in Explorer").clicked() {
                            let dist_name = viewmodel.wsl.distributions[index].name.clone();
                            viewmodel.wsl.open_in_explorer(&dist_name);
                        }

                        if ui.button("⚙️ Settings").clicked() {
                            viewmodel.wsl.status_message =
                                Some("Distribution settings are not yet available".to_string());
                        }
                    }
                });
            }
        }
    } else {
        ui.label(
            "No WSL distributions found. Install WSL and Linux distributions to use this feature.",
        );
    }

    // WSL status message
    if let Some(ref msg) = viewmodel.wsl.status_message {
        ui.separator();
        ui.label(msg);
    }
}
