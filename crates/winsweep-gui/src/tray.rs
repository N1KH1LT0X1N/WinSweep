//! System tray integration for WinSweep

use anyhow::Result;
use eframe::egui;
use std::sync::mpsc;
use tracing::{debug, error, info};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon,
};

/// System tray manager
pub struct TrayManager {
    tray_icon: Option<TrayIcon>,
    event_rx: mpsc::Receiver<TrayEvent>,
    _menu_channel: Option<MenuChannel>,
}

/// Menu channel for handling events
struct MenuChannel {
    _receiver: tray_icon::menu::MenuEventReceiver,
}

impl MenuChannel {
    fn new(event_tx: mpsc::Sender<TrayEvent>) -> Result<Self> {
        let receiver = MenuEvent::receiver();

        // Spawn a thread to handle menu events
        let _receiver_clone = receiver.clone();
        std::thread::spawn(move || {
            while let Ok(event) = _receiver_clone.recv() {
                match event {
                    MenuEvent::MenuItemClick { id } => {
                        // Map menu item IDs to events
                        // The id is a u16, so we need to map it differently
                        let tray_event = match id {
                            0 => TrayEvent::Show,
                            2 => TrayEvent::QuickScan,
                            3 => TrayEvent::CleanTemp,
                            4 => TrayEvent::CleanAll,
                            6 => TrayEvent::Settings,
                            7 => TrayEvent::About,
                            9 => TrayEvent::Quit,
                            _ => continue,
                        };
                        let _ = event_tx.send(tray_event);
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            _receiver: receiver,
        })
    }
}

/// Create a default icon (placeholder)
fn create_default_icon() -> Result<Icon> {
    // For now, create an empty icon as placeholder
    // In production, you would load from actual icon file
    let width = 16;
    let height = 16;
    let rgba = vec![0; (width * height * 4) as usize]; // Transparent icon

    Icon::from_rgba(rgba, width as u32, height as u32).map_err(Into::into)
}

/// System tray events
#[derive(Debug, Clone)]
pub enum TrayEvent {
    Show,
    Hide,
    QuickScan,
    CleanTemp,
    CleanAll,
    Settings,
    About,
    Quit,
}

impl TrayManager {
    /// Create a new system tray manager
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel();

        // Create tray menu items with IDs
        let show_item = MenuItem::new("Show WinSweep", true, Some("show".into()));
        let quick_scan_item = MenuItem::new("Quick Scan", true, Some("quick_scan".into()));
        let clean_temp_item = MenuItem::new("Clean Temp Files", true, Some("clean_temp".into()));
        let clean_all_item = MenuItem::new("Clean All", true, Some("clean_all".into()));
        let settings_item = MenuItem::new("Settings", true, Some("settings".into()));
        let about_item = MenuItem::new("About", true, Some("about".into()));
        let quit_item = MenuItem::new("Quit", true, Some("quit".into()));

        // Create menu with items
        let menu = Menu::with_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &quick_scan_item,
            &clean_temp_item,
            &clean_all_item,
            &PredefinedMenuItem::separator(),
            &settings_item,
            &about_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;

        // Set up menu event handler
        let menu_channel = MenuChannel::new(event_tx.clone())?;

        // Create tray icon with a simple icon (placeholder)
        let icon = create_default_icon()?;
        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(menu)
            .with_tooltip("WinSweep - Disk Cleaning Tool")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            tray_icon: Some(tray_icon),
            event_rx,
            _menu_channel: Some(menu_channel),
        })
    }

    /// Get the next tray event
    pub fn next_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Update tray icon based on operation status
    pub fn update_status(&mut self, is_busy: bool) -> Result<()> {
        if let Some(ref mut tray_icon) = self.tray_icon {
            if is_busy {
                tray_icon.set_icon(include_bytes!("../assets/icon-busy.ico"))?;
                tray_icon.set_tooltip("WinSweep - Working...")?;
            } else {
                tray_icon.set_icon(include_bytes!("../assets/icon.ico"))?;
                tray_icon.set_tooltip("WinSweep - Disk Cleaning Tool")?;
            }
        }
        Ok(())
    }

    /// Show balloon notification
    pub fn show_notification(&mut self, title: &str, message: &str) -> Result<()> {
        if let Some(ref mut tray_icon) = self.tray_icon {
            tray_icon.set_tooltip(message)?;
            // Note: tray-icon crate doesn't support balloon notifications directly
            // For full notification support, we might need to use Windows API directly
        }
        Ok(())
    }

    /// Hide the system tray
    pub fn hide(&mut self) -> Result<()> {
        if let Some(tray_icon) = self.tray_icon.take() {
            tray_icon.hide()?;
        }
        Ok(())
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        let _ = self.hide();
    }
}
