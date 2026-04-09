//! Windows Update view

use eframe::egui;
use crate::viewmodel::WinSweepViewModel;

pub fn show_windows_update(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Windows Update");
    ui.separator();
    
    // Update status
    ui.horizontal(|ui| {
        ui.label("Status:");
        
        if viewmodel.windows_update.update_status.service_running {
            ui.colored_label(egui::Color32::GREEN, "● Update service running");
        } else {
            ui.colored_label(egui::Color32::RED, "● Update service stopped");
        }
        
        ui.separator();
        ui.label(format!("Last check: {}", viewmodel.windows_update.update_status.last_check));
        ui.label(format!("Pending updates: {}", viewmodel.windows_update.update_status.pending_updates));
    });
    
    // Actions
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("🔍 Check for Updates").clicked() {
            viewmodel.windows_update.check_for_updates();
        }
        
        if ui.button("🧹 Clean Update Cache").clicked() {
            viewmodel.windows_update.start_cleanup();
        }
        
        if viewmodel.windows_update.cleanup_in_progress {
            ui.spinner();
            ui.add(egui::ProgressBar::new(viewmodel.windows_update.cleanup_progress));
        }
    });
    
    // Update information
    ui.separator();
    ui.heading("Update Information");
    
    ui.horizontal(|ui| {
        ui.label("Download size:");
        ui.label(format_bytes(viewmodel.windows_update.update_status.download_size));
    });
    
    // Available updates
    if !viewmodel.windows_update.available_updates.is_empty() {
        ui.separator();
        ui.heading("Available Updates");
        
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, update) in viewmodel.windows_update.available_updates.iter().enumerate() {
                    let selected = viewmodel.windows_update.selected_update == Some(i);
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
                            // Category indicator
                            let (color, icon) = match update.category {
                                crate::viewmodel::windows_update::UpdateCategory::Critical => (egui::Color32::RED, "🔴"),
                                crate::viewmodel::windows_update::UpdateCategory::Important => (egui::Color32::YELLOW, "🟡"),
                                crate::viewmodel::windows_update::UpdateCategory::Optional => (egui::Color32::GRAY, "⚪"),
                                crate::viewmodel::windows_update::UpdateCategory::Driver => (egui::Color32::BLUE, "🔵"),
                            };
                            
                            ui.colored_label(color, icon);
                            
                            // Update info
                            ui.vertical(|ui| {
                                ui.label(&update.title);
                                ui.label(format!("Size: {}", format_bytes(update.size)));
                                ui.label(&update.description);
                            });
                            
                            // Status
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if update.installed {
                                    ui.colored_label(egui::Color32::GREEN, "✓ Installed");
                                } else {
                                    if ui.button("Download").clicked() {
                                        // TODO: Download update
                                    }
                                    if ui.button("Install").clicked() {
                                        // TODO: Install update
                                    }
                                }
                            });
                        });
                        
                        if ui.allocate_response(egui::Vec2::ZERO, egui::Sense::click()).clicked() {
                            viewmodel.windows_update.selected_update = Some(i);
                        }
                    });
                }
            });
    } else {
        ui.label("No updates available");
    }
    
    // Cleanup options
    ui.separator();
    ui.heading("Cleanup Options");
    
    ui.checkbox(&mut viewmodel.windows_update.cleanup_options.remove_downloads, "Remove downloaded updates");
    ui.checkbox(&mut viewmodel.windows_update.cleanup_options.compress_backups, "Compress backup files");
    ui.checkbox(&mut viewmodel.windows_update.cleanup_options.remove_old_versions, "Remove old Windows versions");
    
    // Status message
    if let Some(ref msg) = viewmodel.windows_update.status_message {
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
