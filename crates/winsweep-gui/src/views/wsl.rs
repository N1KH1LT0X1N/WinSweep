//! WSL view

use eframe::egui;
use crate::viewmodel::WinSweepViewModel;

pub fn show_wsl(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("WSL Management");
    ui.separator();
    
    // Refresh button
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            if let Some(ref wsl_detector) = viewmodel.wsl_detector() {
                viewmodel.wsl.refresh_distributions(wsl_detector);
            }
        }
        
        ui.label("Windows Subsystem for Linux distributions");
    });
    
    // WSL distributions list
    if !viewmodel.wsl.distributions.is_empty() {
        ui.separator();
        
        // Distribution list
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, dist) in viewmodel.wsl.distributions.iter().enumerate() {
                    let selected = viewmodel.wsl.selected_distribution == Some(i);
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
                            let status_color = match dist.state {
                                winsweep_core::WslState::Running => egui::Color32::GREEN,
                                winsweep_core::WslState::Stopped => egui::Color32::RED,
                                _ => egui::Color32::YELLOW,
                            };
                            
                            ui.colored_label(status_color, "●");
                            
                            // Distribution info
                            ui.vertical(|ui| {
                                ui.label(format!("{} (WSL{})", dist.name, 
                                    match dist.version {
                                        winsweep_core::WslVersion::Wsl1 => "1",
                                        winsweep_core::WslVersion::Wsl2 => "2",
                                        winsweep_core::WslVersion::Unknown => "?",
                                    }
                                ));
                                ui.label(format!("State: {:?}", dist.state));
                                ui.label(format!("Size: {:.1} GB", dist.size_gb));
                                ui.label(format!("Path: {}", dist.path));
                            });
                            
                            // Actions
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if dist.state == winsweep_core::WslState::Running {
                                    if ui.button("⏹ Stop").clicked() {
                                        // TODO: Stop distribution
                                    }
                                } else {
                                    if ui.button("▶ Start").clicked() {
                                        // TODO: Start distribution
                                    }
                                }
                                
                                if ui.button("🗑️ Unregister").clicked() {
                                    // TODO: Unregister distribution
                                }
                            });
                        });
                        
                        if ui.allocate_response(egui::Vec2::ZERO, egui::Sense::click()).clicked() {
                            viewmodel.wsl.selected_distribution = Some(i);
                        }
                    });
                }
            });
        
        // Selected distribution actions
        if let Some(index) = viewmodel.wsl.selected_distribution {
            if index < viewmodel.wsl.distributions.len() {
                ui.separator();
                
                let dist = &viewmodel.wsl.distributions[index];
                ui.horizontal(|ui| {
                    ui.heading(format!("{} Actions", dist.name));
                    
                    if viewmodel.wsl.compact_in_progress {
                        ui.spinner();
                        ui.label("Compacting...");
                        ui.add(egui::ProgressBar::new(viewmodel.wsl.compact_progress));
                    } else {
                        if ui.button("🗜️ Compact Disk").clicked() {
                            viewmodel.wsl.start_compact();
                        }
                        
                        if ui.button("📁 Open in Explorer").clicked() {
                            // TODO: Open distribution path
                        }
                        
                        if ui.button("⚙️ Settings").clicked() {
                            // TODO: Open distribution settings
                        }
                    }
                });
            }
        }
    } else {
        ui.label("No WSL distributions found. Install WSL and Linux distributions to use this feature.");
    }
    
    // WSL status message
    if let Some(ref msg) = viewmodel.wsl.status_message {
        ui.separator();
        ui.label(msg);
    }
}
