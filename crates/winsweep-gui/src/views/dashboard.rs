//! Dashboard view

use crate::viewmodel::WinSweepViewModel;
use crate::views::utils::format_bytes;
use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};

pub fn show_dashboard(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Dashboard");
    ui.separator();

    let si = &viewmodel.dashboard.system_info;

    // ── System Overview ──────────────────────────────────────────────────────
    ui.collapsing("System Overview", |ui| {
        egui::Grid::new("sysinfo_grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label("OS:");
                let os = if si.windows_version.is_empty() {
                    "Detecting…".to_string()
                } else {
                    format!("{} {}", si.windows_edition, si.windows_version)
                };
                ui.label(os);
                ui.end_row();

                ui.label("Disk (primary):");
                let total = si.total_disk_space;
                let free = si.free_disk_space;
                ui.label(if total > 0 {
                    format!(
                        "{} free / {} total",
                        format_bytes(free),
                        format_bytes(total)
                    )
                } else {
                    "Detecting…".to_string()
                });
                ui.end_row();

                ui.label("Memory:");
                ui.label(format!("{:.1}% used", si.memory_usage));
                ui.end_row();

                ui.label("CPU:");
                ui.label(format!("{:.1}% used", si.cpu_usage));
                ui.end_row();
            });
    });

    // ── Quick Actions ────────────────────────────────────────────────────────
    ui.separator();
    ui.heading("Quick Actions");

    ui.horizontal(|ui| {
        if ui.button("🔍 Quick Scan").clicked() {
            viewmodel.set_current_view(crate::viewmodel::NavigationView::Scan);
        }
        if ui.button("🧹 Clean Temp Files").clicked() {
            viewmodel.start_elevated_task(
                crate::elevated_coordinator::ElevatedOperation::CleanSystemTemp {
                    include_user_temp: true,
                    include_system_temp: true,
                },
                "Clean system temp files".to_string(),
            );
        }
        if ui.button("📦 Clean Package Caches").clicked() {
            viewmodel.set_current_view(crate::viewmodel::NavigationView::PackageManagers);
        }
        if ui.button("♻️ Empty Recycle Bin").clicked() {
            viewmodel.empty_recycle_bin();
        }
        if ui.button("🌐 Clean Browser Caches").clicked() {
            viewmodel.start_browser_cache_clean_task();
        }
    });

    // ── Storage Gauge ────────────────────────────────────────────────────────
    ui.separator();
    ui.heading("Storage Usage");

    let total = viewmodel.dashboard.system_info.total_disk_space;
    let free = viewmodel.dashboard.system_info.free_disk_space;

    if total > 0 {
        let used = total - free;
        let used_frac = used as f32 / total as f32;

        // Segmented colour bar painted with egui::Painter
        let bar_size = egui::vec2(ui.available_width(), 28.0);
        let (bar_rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());

        if ui.is_rect_visible(bar_rect) {
            let p = ui.painter();

            // Background
            p.rect_filled(bar_rect, 4.0, egui::Color32::from_rgb(30, 30, 30));

            // Used portion (amber)
            let used_w = bar_rect.width() * used_frac;
            let used_rect =
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(used_w, bar_rect.height()));
            p.rect_filled(used_rect, 4.0, egui::Color32::from_rgb(210, 120, 40));

            // WinSweep reclaimable overlay (red, drawn on top of used portion)
            let bd = &viewmodel.dashboard.category_breakdown;
            let reclaimable = bd.artifact_bytes
                + bd.temp_bytes
                + bd.package_cache_bytes
                + bd.recycle_bin_bytes
                + bd.other_bytes;
            if reclaimable > 0 && reclaimable <= used {
                let recl_frac = reclaimable as f32 / total as f32;
                let recl_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    egui::vec2(bar_rect.width() * recl_frac, bar_rect.height()),
                );
                p.rect_filled(recl_rect, 4.0, egui::Color32::from_rgb(200, 60, 60));
            }

            // Border
            p.rect_stroke(
                bar_rect,
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80)),
            );

            // Label
            p.text(
                bar_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "{} used / {} free  ({:.1}%)",
                    format_bytes(used),
                    format_bytes(free),
                    used_frac * 100.0
                ),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.colored_label(egui::Color32::from_rgb(210, 120, 40), "■ Used");
            ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "■ Reclaimable");
            ui.colored_label(egui::Color32::from_gray(100), "■ Free");
        });
    } else {
        ui.label("Querying disk info…");
    }

    // ── All Drives ────────────────────────────────────────────────────────────
    if !viewmodel.dashboard.drives.is_empty() {
        ui.separator();
        ui.heading("Drives");

        for drive in &viewmodel.dashboard.drives {
            if drive.total_bytes == 0 {
                continue;
            }
            let used_frac = drive.used_bytes as f32 / drive.total_bytes as f32;

            ui.horizontal(|ui| {
                // Drive letter / mount point label
                ui.label(egui::RichText::new(&drive.mount_point).strong().monospace());
                if !drive.name.is_empty() {
                    ui.label(format!("({})", drive.name));
                }
                ui.label(&drive.file_system);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} / {}",
                        format_bytes(drive.free_bytes),
                        format_bytes(drive.total_bytes)
                    ));
                });
            });

            // Mini progress bar per drive
            let bar_size = egui::vec2(ui.available_width(), 10.0);
            let (bar_rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
            if ui.is_rect_visible(bar_rect) {
                let p = ui.painter();
                p.rect_filled(bar_rect, 2.0, egui::Color32::from_rgb(40, 40, 40));
                let used_w = bar_rect.width() * used_frac;
                let color = if used_frac >= 0.95 {
                    egui::Color32::from_rgb(200, 60, 60)
                } else if used_frac >= 0.80 {
                    egui::Color32::from_rgb(210, 140, 40)
                } else {
                    egui::Color32::from_rgb(60, 160, 100)
                };
                let used_rect =
                    egui::Rect::from_min_size(bar_rect.min, egui::vec2(used_w, bar_rect.height()));
                p.rect_filled(used_rect, 2.0, color);
                p.rect_stroke(
                    bar_rect,
                    2.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 70)),
                );
            }
            ui.add_space(4.0);
        }
    }

    // ── Reclaimable Breakdown Chart ──────────────────────────────────────────
    ui.separator();
    {
        let bd = &viewmodel.dashboard.category_breakdown;
        let total_reclaimable = bd.artifact_bytes
            + bd.temp_bytes
            + bd.package_cache_bytes
            + bd.recycle_bin_bytes
            + bd.other_bytes;
        ui.heading(format!("{} reclaimable", format_bytes(total_reclaimable)));
    }
    ui.heading("Reclaimable Space by Category");

    {
        let bd = &viewmodel.dashboard.category_breakdown;
        let mb = |b: u64| b as f64 / 1_048_576.0;

        let bars = vec![
            Bar::new(0.0, mb(bd.artifact_bytes))
                .fill(egui::Color32::from_rgb(200, 60, 60))
                .name("Artifacts"),
            Bar::new(1.0, mb(bd.temp_bytes))
                .fill(egui::Color32::from_rgb(80, 160, 210))
                .name("Temp Files"),
            Bar::new(2.0, mb(bd.package_cache_bytes))
                .fill(egui::Color32::from_rgb(80, 200, 120))
                .name("Package Cache"),
            Bar::new(3.0, mb(bd.recycle_bin_bytes))
                .fill(egui::Color32::from_rgb(100, 200, 100))
                .name("Recycle Bin"),
            Bar::new(4.0, mb(bd.other_bytes))
                .fill(egui::Color32::from_rgb(200, 160, 60))
                .name("Other"),
        ];

        let chart = BarChart::new(bars).width(0.6).name("Reclaimable (MB)");

        Plot::new("category_chart")
            .height(160.0)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .y_axis_label("MB")
            .x_axis_label("Category")
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });
    }

    // ── Recent Activity ──────────────────────────────────────────────────────
    ui.separator();
    ui.heading("Recent Activity");

    egui::ScrollArea::vertical()
        .id_salt("recent_activity")
        .max_height(180.0)
        .show(ui, |ui| {
            let ops = &viewmodel.dashboard.recent_operations;
            if ops.is_empty() {
                ui.label("No operations recorded yet.");
            } else {
                for op in ops.iter().rev() {
                    let icon = if op.success { "✅" } else { "❌" };
                    ui.label(format!(
                        "{} {}  —  {}  freed  [{}]",
                        icon,
                        op.operation,
                        format_bytes(op.space_freed),
                        op.timestamp
                    ));
                }
            }
        });

    // ── System Health ────────────────────────────────────────────────────────
    ui.separator();
    ui.heading("System Health");

    let mem = viewmodel.dashboard.system_info.memory_usage;
    let cpu = viewmodel.dashboard.system_info.cpu_usage;
    let disk_pct = if total > 0 {
        (total - free) as f32 / total as f32 * 100.0
    } else {
        0.0
    };

    let health_icon = |pct: f32, warn: f32, crit: f32| {
        if pct >= crit {
            "🔴"
        } else if pct >= warn {
            "🟡"
        } else {
            "🟢"
        }
    };

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(format!(
                "{} Memory: {:.1}%",
                health_icon(mem, 75.0, 90.0),
                mem
            ));
            ui.label(format!("{} CPU: {:.1}%", health_icon(cpu, 75.0, 90.0), cpu));
        });
        ui.vertical(|ui| {
            ui.label(format!(
                "{} Disk: {:.1}% used",
                health_icon(disk_pct, 80.0, 95.0),
                disk_pct
            ));
            ui.label("🟢 Status: Monitoring");
        });
    });
}
