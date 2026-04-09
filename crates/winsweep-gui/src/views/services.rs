//! Services view

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

pub fn show_services(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Windows Services");
    ui.separator();

    // Search and filter
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut viewmodel.services.filter_text);

        ui.checkbox(&mut viewmodel.services.show_running_only, "Running only");

        if ui.button("🔄 Refresh").clicked() {
            // TODO: Refresh services
        }
    });

    // Services list
    let filtered_services = viewmodel.services.filtered_services();

    if !filtered_services.is_empty() {
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label("Service Name");
                    ui.label("Status");
                    ui.label("Start Type");
                    ui.label("Actions");
                });
                ui.separator();

                // Services
                for (i, service) in filtered_services.iter().enumerate() {
                    let selected = viewmodel.services.selected_service == Some(i);
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
                            let status_color = match service.status {
                                winsweep_core::ServiceStatus::Running => egui::Color32::GREEN,
                                winsweep_core::ServiceStatus::Stopped => egui::Color32::RED,
                                _ => egui::Color32::YELLOW,
                            };

                            ui.colored_label(status_color, "●");

                            // Service info
                            ui.vertical(|ui| {
                                ui.label(&service.display_name);
                                ui.label(&service.name);
                                ui.label(&service.description);
                            });

                            // Status and start type
                            ui.label(format!("{:?}", service.status));
                            ui.label(format!("{:?}", service.start_type));

                            // Actions
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if service.can_stop
                                        && matches!(
                                            service.status,
                                            winsweep_core::ServiceStatus::Running
                                        )
                                    {
                                        if ui.button("⏹").clicked() {
                                            // TODO: Stop service
                                        }
                                    }

                                    if service.can_start
                                        && !matches!(
                                            service.status,
                                            winsweep_core::ServiceStatus::Running
                                        )
                                    {
                                        if ui.button("▶").clicked() {
                                            // TODO: Start service
                                        }
                                    }

                                    if ui.button("🔄").clicked() {
                                        // TODO: Restart service
                                    }
                                },
                            );
                        });

                        if ui
                            .allocate_response(egui::Vec2::ZERO, egui::Sense::click())
                            .clicked()
                        {
                            // Find the actual index in the full list
                            viewmodel.services.selected_service = viewmodel
                                .services
                                .services
                                .iter()
                                .position(|s| s.name == service.name);
                        }
                    });
                }
            });

        // Selected service actions
        if let Some(index) = viewmodel.services.selected_service {
            if index < viewmodel.services.services.len() {
                ui.separator();

                let service = &viewmodel.services.services[index];
                ui.horizontal(|ui| {
                    ui.heading(format!("{} Actions", service.display_name));

                    if matches!(service.status, winsweep_core::ServiceStatus::Running) {
                        if ui.button("⏹ Stop").clicked() {
                            // TODO: Stop service
                        }
                    } else {
                        if ui.button("▶ Start").clicked() {
                            // TODO: Start service
                        }
                    }

                    if ui.button("🔄 Restart").clicked() {
                        // TODO: Restart service
                    }

                    if ui.button("⚙️ Properties").clicked() {
                        // TODO: Show service properties
                    }
                });
            }
        }
    } else {
        ui.label("No services found matching the current filter.");
    }

    // Status message
    if let Some(ref msg) = viewmodel.services.status_message {
        ui.separator();
        ui.label(msg);
    }
}
