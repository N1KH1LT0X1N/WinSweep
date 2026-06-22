//! Settings view

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

pub fn show_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Settings");
    ui.separator();

    // Settings categories
    ui.horizontal(|ui| {
        for (i, category) in viewmodel.settings.categories.iter().enumerate() {
            let category_name = match category {
                crate::viewmodel::settings::SettingCategory::General => "General",
                crate::viewmodel::settings::SettingCategory::Scan => "Scan",
                crate::viewmodel::settings::SettingCategory::Cleanup => "Cleanup",
                crate::viewmodel::settings::SettingCategory::Notifications => "Notifications",
                crate::viewmodel::settings::SettingCategory::Advanced => "Advanced",
            };

            if ui
                .selectable_label(
                    viewmodel.settings.selected_category == Some(i),
                    category_name,
                )
                .clicked()
            {
                viewmodel.settings.selected_category = Some(i);
            }
        }
    });

    ui.separator();

    // Content based on selected category
    if let Some(category) = viewmodel
        .settings
        .selected_category
        .and_then(|i| viewmodel.settings.categories.get(i))
    {
        match category {
            crate::viewmodel::settings::SettingCategory::General => {
                show_general_settings(ui, viewmodel);
            }
            crate::viewmodel::settings::SettingCategory::Scan => {
                show_scan_settings(ui, viewmodel);
            }
            crate::viewmodel::settings::SettingCategory::Cleanup => {
                show_cleanup_settings(ui, viewmodel);
            }
            crate::viewmodel::settings::SettingCategory::Notifications => {
                show_notification_settings(ui, viewmodel);
            }
            crate::viewmodel::settings::SettingCategory::Advanced => {
                show_advanced_settings(ui, viewmodel);
            }
        }
    }

    // Save/Reset buttons
    ui.separator();
    ui.horizontal(|ui| {
        if viewmodel.settings.has_unsaved_changes {
            ui.colored_label(egui::Color32::YELLOW, "⚠ You have unsaved changes");
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Reset to Defaults").clicked() {
                viewmodel.settings.reset_to_defaults();
            }

            if ui
                .add_enabled(
                    viewmodel.settings.has_unsaved_changes,
                    egui::Button::new("💾 Save Settings"),
                )
                .clicked()
                && viewmodel.settings.save_settings().is_ok()
            {
                let updated = viewmodel.settings.config().clone();
                viewmodel.set_config(updated);
            }
        });
    });

    // Status message
    if let Some(ref msg) = viewmodel.settings.status_message {
        ui.separator();
        ui.label(msg);
    }
}

fn show_general_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("General Settings");

    let mut start_with_windows = viewmodel.settings.config().ui.start_with_windows;
    if ui
        .checkbox(&mut start_with_windows, "Start WinSweep with Windows")
        .changed()
    {
        viewmodel
            .settings
            .set_start_with_windows(start_with_windows);
    }

    ui.checkbox(
        &mut viewmodel.settings.config_mut().ui.minimize_to_tray,
        "Minimize to system tray",
    );

    ui.checkbox(
        &mut viewmodel.settings.config_mut().ui.show_notifications,
        "Show notifications",
    );

    ui.separator();

    ui.label("Language:");
    ui.horizontal(|ui| {
        let lang = &mut viewmodel.settings.config_mut().ui.language;
        egui::ComboBox::from_label("")
            .selected_text(match lang.as_str() {
                "es" => "Español",
                "fr" => "Français",
                _ => "English (US)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(lang, "en".to_string(), "English (US)");
                ui.selectable_value(lang, "es".to_string(), "Español");
                ui.selectable_value(lang, "fr".to_string(), "Français");
            });
    });

    ui.separator();

    ui.label("Theme:");
    ui.horizontal(|ui| {
        let theme = &mut viewmodel.settings.config_mut().ui.theme;
        egui::ComboBox::from_label("")
            .selected_text(theme.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(theme, "dark".to_string(), "Dark");
                ui.selectable_value(theme, "light".to_string(), "Light");
                ui.selectable_value(theme, "system".to_string(), "System");
            });
    });
}

fn show_scan_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Scan Settings");

    ui.checkbox(
        &mut viewmodel.settings.config_mut().scan_include_hidden,
        "Include hidden files and folders",
    );

    ui.checkbox(
        &mut viewmodel.settings.config_mut().scan_include_system,
        "Include system files and folders",
    );

    ui.separator();

    ui.label("Default scan locations:");
    let mut to_remove = None;
    let paths: Vec<std::path::PathBuf> = viewmodel.settings.config().scan.paths.clone();
    for (i, path) in paths.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(path.display().to_string());
            if ui.button("🗑").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        viewmodel.settings.config_mut().scan.paths.remove(i);
        viewmodel.settings.has_unsaved_changes = true;
    }
    ui.horizontal(|ui| {
        if ui.button("Add Location").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                viewmodel.settings.config_mut().scan.paths.push(folder);
                viewmodel.settings.has_unsaved_changes = true;
            }
        }

        if ui.button("Reset to Defaults").clicked() {
            viewmodel.settings.config_mut().scan.paths =
                winsweep_common::types::ScanConfig::default().paths;
            viewmodel.settings.has_unsaved_changes = true;
        }
    });

    ui.separator();

    ui.label("File size filter:");
    ui.horizontal(|ui| {
        ui.label("Minimum size:");
        ui.add(
            egui::DragValue::new(&mut viewmodel.settings.config_mut().scan_min_size).suffix(" MB"),
        );
    });
}

fn show_cleanup_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Cleanup Settings");

    ui.checkbox(
        &mut viewmodel
            .settings
            .config_mut()
            .cleanup
            .cleanup_confirm_delete,
        "Confirm before deleting files",
    );

    ui.checkbox(
        &mut viewmodel
            .settings
            .config_mut()
            .cleanup
            .cleanup_move_to_recycle,
        "Move files to recycle bin instead of permanent deletion",
    );

    ui.separator();

    ui.label("Automatic cleanup:");
    ui.checkbox(
        &mut viewmodel.settings.config_mut().auto_cleanup_enabled,
        "Enable automatic cleanup",
    );

    ui.horizontal(|ui| {
        ui.label("Run every:");
        ui.add(
            egui::DragValue::new(&mut viewmodel.settings.config_mut().auto_cleanup_days)
                .suffix(" days"),
        );
    });

    if let Some(ref last) = viewmodel.dashboard.last_auto_cleanup {
        ui.label(format!("Last run: {}", last));
    } else {
        ui.label("Last run: Never");
    }

    ui.separator();
    ui.label("Windows Task Scheduler:");
    let task_exists = crate::scheduler::task_exists();
    ui.horizontal(|ui| {
        if task_exists {
            ui.colored_label(egui::Color32::GREEN, "✔ Scheduled task registered");
            if ui.button("Remove Task").clicked() {
                match crate::scheduler::remove_task() {
                    Ok(msg) => viewmodel.settings.status_message = Some(msg),
                    Err(e) => viewmodel.settings.status_message = Some(format!("Error: {}", e)),
                }
            }
        } else {
            ui.colored_label(egui::Color32::GRAY, "○ No scheduled task");
            if ui.button("Register Startup Task").clicked() {
                if let Some(exe) = crate::scheduler::current_exe() {
                    let freq = crate::scheduler::TaskFrequency::from_days(
                        viewmodel.settings.config().auto_cleanup_days,
                    );
                    match crate::scheduler::register_task(&exe, freq) {
                        Ok(msg) => viewmodel.settings.status_message = Some(msg),
                        Err(e) => viewmodel.settings.status_message = Some(format!("Error: {}", e)),
                    }
                }
            }
        }
    });

    ui.separator();

    ui.label("What to clean:");
    ui.checkbox(
        &mut viewmodel.settings.config_mut().cleanup.clean_temp_files,
        "Temporary files",
    );
    ui.checkbox(
        &mut viewmodel.settings.config_mut().cleanup.clean_recycle_bin,
        "Recycle bin",
    );
    ui.checkbox(
        &mut viewmodel.settings.config_mut().cleanup.clean_prefetch,
        "Prefetch files",
    );
    ui.checkbox(
        &mut viewmodel.settings.config_mut().cleanup.clean_browser_cache,
        "Browser cache",
    );
}

fn show_notification_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Notification Settings");

    ui.checkbox(
        &mut viewmodel.settings.config_mut().notify_cleanup_complete,
        "Notify when cleanup is complete",
    );

    ui.checkbox(
        &mut viewmodel.settings.config_mut().notify_low_disk_space,
        "Notify when disk space is low",
    );

    ui.separator();

    ui.label("Low disk space threshold:");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(
                &mut viewmodel.settings.config_mut().low_disk_threshold,
                1..=50,
            )
            .text("Percentage"),
        );
    });

    ui.separator();

    ui.label("Notification duration:");
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut viewmodel.settings.config_mut().notification_duration)
                .suffix(" seconds"),
        );
    });
}

fn show_advanced_settings(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Advanced Settings");

    ui.checkbox(
        &mut viewmodel.settings.config_mut().logging.debug_mode,
        "Enable debug mode",
    );

    ui.checkbox(
        &mut viewmodel.settings.config_mut().logging.verbose_logging,
        "Enable verbose logging",
    );

    ui.separator();

    ui.label("Performance:");
    ui.horizontal(|ui| {
        ui.label("Max concurrent operations:");
        ui.add(
            egui::DragValue::new(&mut viewmodel.settings.config_mut().max_concurrent_ops)
                .range(1..=16),
        );
    });

    ui.separator();

    ui.label("Data Management:");
    if ui.button("Export Settings").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML", &["toml"])
            .set_file_name("winsweep-settings.toml")
            .save_file()
        {
            if let Err(e) = viewmodel.settings.export_settings(&path) {
                viewmodel.settings.status_message = Some(format!("Export failed: {}", e));
            } else {
                viewmodel.settings.status_message = Some("Settings exported".to_string());
            }
        }
    }

    if ui.button("Import Settings").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML", &["toml"])
            .pick_file()
        {
            if let Err(e) = viewmodel.settings.import_settings(&path) {
                viewmodel.settings.status_message = Some(format!("Import failed: {}", e));
            } else {
                viewmodel.settings.status_message = Some("Settings imported".to_string());
            }
        }
    }

    ui.separator();

    ui.colored_label(egui::Color32::RED, "⚠ Danger Zone:");
    if ui.button("Reset All Settings").clicked() {
        viewmodel.settings.reset_to_defaults();
    }

    if ui.button("Clear All Data").clicked() {
        viewmodel.settings.clear_all_data();
        viewmodel.dashboard.recent_operations.clear();
    }
}
