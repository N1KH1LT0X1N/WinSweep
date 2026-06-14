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
            viewmodel.start_service_refresh_task();
        }
    });

    // Services list
    let filtered_services: Vec<_> = viewmodel
        .services
        .filtered_services()
        .into_iter()
        .cloned()
        .collect();

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
                            let status_color = match service.status.current_state {
                                winsweep_core::ServiceState::Running => egui::Color32::GREEN,
                                winsweep_core::ServiceState::Stopped => egui::Color32::RED,
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
                                            service.status.current_state,
                                            winsweep_core::ServiceState::Running
                                        )
                                        && ui.button("⏹").clicked()
                                    {
                                        viewmodel.stop_service_task(service.name.clone());
                                    }

                                    if service.can_start
                                        && !matches!(
                                            service.status.current_state,
                                            winsweep_core::ServiceState::Running
                                        )
                                        && ui.button("▶").clicked()
                                    {
                                        viewmodel.start_service_task(service.name.clone());
                                    }

                                    if ui.button("🔄").clicked() {
                                        viewmodel.restart_service_task(service.name.clone());
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
                let display_name = service.display_name.clone();
                let service_name = service.name.clone();
                let is_running = matches!(
                    service.status.current_state,
                    winsweep_core::ServiceState::Running
                );
                let start_type = service.start_type.clone();
                let can_stop = service.can_stop;
                let can_start = service.can_start;
                ui.horizontal(|ui| {
                    ui.heading(format!("{} Actions", display_name));

                    if is_running {
                        if ui.button("⏹ Stop").clicked() {
                            viewmodel.stop_service_task(service_name.clone());
                        }
                    } else {
                        if ui.button("▶ Start").clicked() {
                            viewmodel.start_service_task(service_name.clone());
                        }
                    }

                    if ui.button("🔄 Restart").clicked() {
                        viewmodel.restart_service_task(service_name.clone());
                    }

                    if ui.button("⚙️ Properties").clicked() {
                        viewmodel.services.status_message = Some(format!(
                            "{} | Start: {:?} | Can stop: {} | Can start: {}",
                            service_name, start_type, can_stop, can_start
                        ));
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
