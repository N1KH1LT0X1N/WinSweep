//! Docker view

use crate::viewmodel::WinSweepViewModel;
use crate::views::utils::format_bytes;
use eframe::egui;

pub fn show_docker(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Docker Cleanup");
    ui.separator();

    // Refresh and status
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            viewmodel.start_docker_refresh_task();
        }

        // Docker daemon status
        if let Some(docker_client) = viewmodel.docker_client() {
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
        if ui
            .selectable_label(
                matches!(
                    viewmodel.docker.selected_tab,
                    crate::viewmodel::DockerTab::Containers
                ),
                "Containers",
            )
            .clicked()
        {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Containers;
        }
        if ui
            .selectable_label(
                matches!(
                    viewmodel.docker.selected_tab,
                    crate::viewmodel::DockerTab::Images
                ),
                "Images",
            )
            .clicked()
        {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Images;
        }
        if ui
            .selectable_label(
                matches!(
                    viewmodel.docker.selected_tab,
                    crate::viewmodel::DockerTab::Volumes
                ),
                "Volumes",
            )
            .clicked()
        {
            viewmodel.docker.selected_tab = crate::viewmodel::DockerTab::Volumes;
        }
        if ui
            .selectable_label(
                matches!(
                    viewmodel.docker.selected_tab,
                    crate::viewmodel::DockerTab::Networks
                ),
                "Networks",
            )
            .clicked()
        {
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
        if viewmodel.is_operation_running() {
            ui.spinner();
            ui.add(egui::ProgressBar::new(
                viewmodel.operation_progress().unwrap_or(0.0),
            ));
        } else {
            if ui.button("🗑️ Clean All").clicked() {
                let tab = match viewmodel.docker.selected_tab {
                    crate::viewmodel::DockerTab::Containers => "containers",
                    crate::viewmodel::DockerTab::Images => "images",
                    crate::viewmodel::DockerTab::Volumes => "volumes",
                    crate::viewmodel::DockerTab::Networks => "networks",
                };
                viewmodel.start_docker_prune_task(tab);
            }

            if ui.button("🧹 Prune System").clicked() {
                viewmodel.start_docker_prune_task("containers");
                viewmodel.start_docker_prune_task("images");
                viewmodel.start_docker_prune_task("volumes");
                viewmodel.start_docker_prune_task("networks");
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
                        let id = container.id.clone();
                        if matches!(
                            container.status,
                            winsweep_core::docker::ContainerStatus::Running
                        ) && ui.button("⏹").clicked()
                        {
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("docker")
                                    .args(["stop", &id])
                                    .output();
                            });
                        }
                        let id2 = container.id.clone();
                        if ui.button("🗑️").clicked() {
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("docker")
                                    .args(["rm", "-f", &id2])
                                    .output();
                            });
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
                        let id = image.id.clone();
                        if ui.button("🗑️").clicked() {
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("docker")
                                    .args(["rmi", "-f", &id])
                                    .output();
                            });
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
                    ui.label(volume.mount_point.display().to_string());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let name = volume.name.clone();
                        if ui.button("🗑️").clicked() {
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("docker")
                                    .args(["volume", "rm", "-f", &name])
                                    .output();
                            });
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
                        let name = network.name.clone();
                        if ui.button("🗑️").clicked() {
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("docker")
                                    .args(["network", "rm", &name])
                                    .output();
                            });
                        }
                    });
                });
            }
        });
}
