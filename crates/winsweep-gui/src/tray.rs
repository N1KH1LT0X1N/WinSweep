//! System tray integration for WinSweep

use anyhow::Result;
use std::sync::mpsc;
use tracing::info;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon,
};

/// System tray events
#[derive(Debug, Clone)]
pub enum TrayEvent {
    Show,
    QuickScan,
    CleanTemp,
    CleanAll,
    Settings,
    About,
    Quit,
}

/// System tray manager
pub struct TrayManager {
    tray_icon: Option<TrayIcon>,
    event_rx: mpsc::Receiver<TrayEvent>,
}

/// Solid blue 16×16 RGBA icon — shown when idle
fn create_idle_icon() -> Result<Icon> {
    let mut rgba = vec![0u8; 16 * 16 * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = 0x00; // R
        px[1] = 0x78; // G
        px[2] = 0xFF; // B
        px[3] = 0xFF; // A
    }
    Icon::from_rgba(rgba, 16, 16).map_err(Into::into)
}

impl TrayManager {
    /// Create a new system tray manager
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel();

        let show_item = MenuItem::new("Show WinSweep", true, None);
        let quick_scan_item = MenuItem::new("Quick Scan", true, None);
        let clean_temp_item = MenuItem::new("Clean Temp Files", true, None);
        let clean_all_item = MenuItem::new("Clean All", true, None);
        let settings_item = MenuItem::new("Settings", true, None);
        let about_item = MenuItem::new("About", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        // Capture stable MenuIds before the items are moved into the Menu
        let show_id = show_item.id().clone();
        let quick_scan_id = quick_scan_item.id().clone();
        let clean_temp_id = clean_temp_item.id().clone();
        let clean_all_id = clean_all_item.id().clone();
        let settings_id = settings_item.id().clone();
        let about_id = about_item.id().clone();
        let quit_id = quit_item.id().clone();

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

        // Forward menu events to the mpsc channel via a static event-handler
        // (avoids needing to hold onto the crossbeam Receiver)
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let tray_event = if event.id == show_id {
                TrayEvent::Show
            } else if event.id == quick_scan_id {
                TrayEvent::QuickScan
            } else if event.id == clean_temp_id {
                TrayEvent::CleanTemp
            } else if event.id == clean_all_id {
                TrayEvent::CleanAll
            } else if event.id == settings_id {
                TrayEvent::Settings
            } else if event.id == about_id {
                TrayEvent::About
            } else if event.id == quit_id {
                TrayEvent::Quit
            } else {
                return;
            };
            let _ = event_tx.send(tray_event);
        }));

        let icon = create_idle_icon()?;
        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("WinSweep - Disk Cleaning Tool")
            .with_icon(icon)
            .build()?;

        info!("System tray initialised");

        Ok(Self {
            tray_icon: Some(tray_icon),
            event_rx,
        })
    }

    /// Poll for the next tray event (non-blocking)
    pub fn next_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Hide the tray icon without destroying it
    pub fn hide(&self) -> Result<()> {
        if let Some(ref tray_icon) = self.tray_icon {
            tray_icon.set_visible(false)?;
        }
        Ok(())
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        let _ = self.hide();
        // Remove the global event handler so the captured Sender is released
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}
