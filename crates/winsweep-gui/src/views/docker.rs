//! Docker view

use eframe::egui;
use crate::viewmodel::WinSweepViewModel;

pub fn show_docker(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Docker Cleanup");
    ui.separator();
    
    // Refresh and status
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            if let Some(ref docker_client) = viewmodel.docker_client() {
                let client = docker_client.clone();
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    // TODO: Refresh resources asynchronously
                    ctx.request_repaint();
                });
            }
        }
        
        // Docker daemon status
        if let Some(ref docker_client) = viewmodel.docker_client() {
            if docker_client.is_daemon_running() {
                ui.colored_label(egui::Color32::GREEN, "● Docker daemon running");
            } else {
                ui.colored_label(egui::Color32::RED, "● Docker daemon not running");
            }
        } else {
            ui.colored_label(egui::Color32::RED, "● Docker not found");
        }
    });
    
    // Tabs
    ui.horizontal(|ui| {
        if ui.selectable_label(matches!(viewmodel.docker.selected_tab, crate::viewmodel::DockerTab::Containers), "Containers").clicked() {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Containers;
        }
        if ui.selectable_label(matches!(viewmodel.docker.selected_tab, crate::viewmodel::DockerTab::Images), "Images").clicked() {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Images;
        }
        if ui.selectable_label(matches!(viewmodel.docker.selected_tab, crate::viewmodel::DockerTab::Volumes), "Volumes").clicked() {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Volumes;
        }
        if ui.selectable_label(matches!(viewmodel.docker.selected_tab, crate::viewmodel::DockerTab::Networks), "Networks").clicked() {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Networks;
        }
    });
    
    ui.separator();
    
    // Content based on selected tab
    match viewmodel.docker.selected_tab {
        crate::viewmodel::DockerTab::Containers => {
            show_containers(ui, viewmodel);
        }
        crate::viewmodel::DockerTab::Images => {
            show_images(ui, viewmodel);
        }
        crate::viewmodel::DockerTab::Volumes => {
            show_volumes(ui, viewmodel);
        }
        crate::viewmodel::DockerTab::Networks => {
            show_networks(ui, viewmodel);
        }
    }
    
    // Cleanup actions
    ui.separator();
    ui.horizontal(|ui| {
        if viewmodel.docker.operation_in_progress {
            ui.spinner();
            ui.add(egui::ProgressBar::new(viewmodel.docker.operation_progress));
        } else {
            if ui.button("🗑️ Clean Selected").clicked() {
                // TODO: Clean selected resources
            }
            
            if ui.button("🗑️ Clean All").clicked() {
                // TODO: Clean all resources
            }
            
            if ui.button("🧹 Prune System").clicked() {
                // TODO: System prune
            }
        }
    });
    
    // Status message
    if let Some(ref msg) = viewmodel.docker.status_message {
        ui.label(msg);
    }
}

fn show_containers(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Containers");
    
    if viewmodel.docker.resources.containers.is_empty() {
        ui.label("No containers found");
        return;
    }
    
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for container in &viewmodel.docker.resources.containers {
                let status_color = match container.status {
                    winsweep_core::docker::ContainerStatus::Running => egui::Color32::GREEN,
                    winsweep_core::docker::ContainerStatus::Exited => egui::Color32::RED,
                    _ => egui::Color32::YELLOW,
                };
                
                ui.horizontal(|ui| {
                    ui.colored_label(status_color, "●");
                    ui.label(&container.name);
                    ui.label(&container.image);
                    ui.label(format!("{:?}", container.status));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if matches!(container.status, winsweep_core::docker::ContainerStatus::Running) {
                            if ui.button("⏹").clicked() {
                                // TODO: Stop container
                            }
                        }
                        if ui.button("🗑️").clicked() {
                            // TODO: Remove container
                        }
                    });
                });
            }
        });
}

fn show_images(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Images");
    
    if viewmodel.docker.resources.images.is_empty() {
        ui.label("No images found");
        return;
    }
    
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for image in &viewmodel.docker.resources.images {
                ui.horizontal(|ui| {
                    if image.dangling {
                        ui.colored_label(egui::Color32::GRAY, "◯");
                    } else {
                        ui.colored_label(egui::Color32::GREEN, "●");
                    }
                    
                    ui.label(format!("{}:{}", image.repository, image.tag));
                    ui.label(format_bytes(image.size));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🗑️").clicked() {
                            // TODO: Remove image
                        }
                    });
                });
            }
        });
}

fn show_volumes(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Volumes");
    
    if viewmodel.docker.resources.volumes.is_empty() {
        ui.label("No volumes found");
        return;
    }
    
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for volume in &viewmodel.docker.resources.volumes {
                ui.horizontal(|ui| {
                    ui.label(&volume.name);
                    ui.label(&volume.driver);
                    ui.label(&volume.mount_point.display().to_string());
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🗑️").clicked() {
                            // TODO: Remove volume
                        }
                    });
                });
            }
        });
}

fn show_networks(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Networks");
    
    if viewmodel.docker.resources.networks.is_empty() {
        ui.label("No networks found");
        return;
    }
    
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for network in &viewmodel.docker.resources.networks {
                // Skip default networks
                if network.name == "bridge" || network.name == "host" || network.name == "none" {
                    continue;
                }
                
                ui.horizontal(|ui| {
                    ui.label(&network.name);
                    ui.label(&network.driver);
                    ui.label(format!("Internal: {}", network.internal));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🗑️").clicked() {
                            // TODO: Remove network
                        }
                    });
                });
            }
        });
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
