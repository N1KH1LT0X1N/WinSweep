//! Dashboard view

use eframe::egui;
use crate::viewmodel::WinSweepViewModel;

pub fn show_dashboard(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel) {
    ui.heading("Dashboard");
    ui.separator();
    
    // System Overview
    ui.collapsing("System Overview", |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Windows Version:");
                ui.label("Edition:");
                ui.label("Disk Space:");
            });
            
            ui.vertical(|ui| {
                ui.label("Windows 11");
                ui.label("Pro");
                ui.label("256 GB / 512 GB");
            });
        });
    });
    
    // Quick Actions
    ui.separator();
    ui.heading("Quick Actions");
    
    ui.horizontal(|ui| {
        if ui.button("🔍 Quick Scan").clicked() {
            viewmodel.set_current_view(crate::viewmodel::NavigationView::Scan);
        }
        
        if ui.button("🧹 Clean Temp Files").clicked() {
            viewmodel.set_status_message(Some("Cleaning temporary files...".to_string()));
        }
        
        if ui.button("📦 Clean Package Caches").clicked() {
            viewmodel.set_current_view(crate::viewmodel::NavigationView::PackageManagers);
        }
    });
    
    // Storage Usage
    ui.separator();
    ui.heading("Storage Usage");
    
    let total_space = 512.0 * 1024.0 * 1024.0 * 1024.0; // 512 GB
    let used_space = 256.0 * 1024.0 * 1024.0 * 1024.0; // 256 GB
    let usage_percent = (used_space / total_space * 100.0) as f32;
    
    ui.add(egui::ProgressBar::new(usage_percent / 100.0).text("50.0% used"));
    
    ui.horizontal(|ui| {
        ui.label("Categories:");
        ui.separator();
        ui.label("System Files: 120 GB");
        ui.label("Applications: 80 GB");
        ui.label("Documents: 40 GB");
        ui.label("Other: 16 GB");
    });
    
    // Recent Activity
    ui.separator();
    ui.heading("Recent Activity");
    
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            ui.label("• Cleaned temporary files - 2.3 GB freed");
            ui.label("• Compacted WSL Ubuntu - 5.1 GB freed");
            ui.label("• Removed Docker containers - 1.2 GB freed");
            ui.label("• Cleaned npm cache - 450 MB freed");
            ui.label("• Windows Update cleanup - 3.7 GB freed");
        });
    
    // System Health
    ui.separator();
    ui.heading("System Health");
    
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("🟢 Windows Update: Up to date");
            ui.label("🟢 WSL: Running normally");
            ui.label("🟡 Docker: Cache needs cleaning");
            ui.label("🟢 Services: All running");
        });
        
        ui.vertical(|ui| {
            ui.label("🟢 Disk Health: Good");
            ui.label("🟢 Memory Usage: 45%");
            ui.label("🟢 CPU Usage: 12%");
            ui.label("🟢 Temperature: Normal");
        });
    });
}
