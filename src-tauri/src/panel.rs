use crate::settings::Language;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
#[cfg(target_os = "windows")]
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// All coordinates are physical pixels, including work area and scaled panel size.
pub fn position(
    anchor: (f64, f64, f64, f64),
    work: (f64, f64, f64, f64),
    panel: (f64, f64),
    gap: f64,
    below: bool,
) -> (f64, f64) {
    let (ax, ay, aw, ah) = anchor;
    let (wx, wy, ww, wh) = work;
    let x = (ax + aw / 2. - panel.0 / 2.).clamp(wx, (wx + ww - panel.0).max(wx));
    let y = if below {
        ay + ah + gap
    } else {
        ay - panel.1 - gap
    };
    (x, y.clamp(wy, (wy + wh - panel.1).max(wy)))
}

pub fn show(app: &tauri::AppHandle, tab: Option<&str>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Some(tray) = app.tray_by_id("fontcatch") {
        if let Ok(Some(rect)) = tray.rect() {
            let pos = rect.position.to_physical::<f64>(1.0);
            if let Ok(Some(monitor)) = window.monitor_from_point(pos.x, pos.y) {
                let scale = monitor.scale_factor();
                let pos = rect.position.to_physical::<f64>(scale);
                let size = rect.size.to_physical::<f64>(scale);
                let work = monitor.work_area();
                let (x, y) = position(
                    (pos.x, pos.y, size.width, size.height),
                    (
                        work.position.x as f64,
                        work.position.y as f64,
                        work.size.width as f64,
                        work.size.height as f64,
                    ),
                    (360. * scale, 480. * scale),
                    8. * scale,
                    cfg!(target_os = "macos"),
                );
                let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
            }
        }
    }
    let _ = window.show();
    let _ = window.set_focus();
    if let Some(tab) = tab {
        let _ = window.emit("navigate", tab);
    }
}

pub fn menu(app: &tauri::AppHandle, language: &Language) -> tauri::Result<Menu<tauri::Wry>> {
    let fr = *language == Language::Fr;
    let open = MenuItem::with_id(
        app,
        "open",
        if fr {
            "Ouvrir Fontcatch"
        } else {
            "Open Fontcatch"
        },
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        app,
        "settings",
        if fr { "Réglages…" } else { "Settings…" },
        true,
        None::<&str>,
    )?;
    let coffee = MenuItem::with_id(
        app,
        "coffee",
        if fr {
            "Soutenir le projet ☕"
        } else {
            "Buy me a coffee ☕"
        },
        crate::configured_url("coffeeUrl").is_some(),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        if fr {
            "Quitter Fontcatch"
        } else {
            "Quit Fontcatch"
        },
        true,
        None::<&str>,
    )?;
    Menu::with_items(
        app,
        &[
            &open,
            &settings,
            &coffee,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}
fn icon() -> &'static [u8] {
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let light = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
            .and_then(|k| k.get_value::<u32, _>("SystemUsesLightTheme"))
            .unwrap_or(0)
            == 1;
        if !light {
            return include_bytes!("../icons/tray/white-44.png");
        }
    }
    #[cfg(target_os = "linux")]
    {
        return include_bytes!("../icons/tray/white-44.png");
    }
    #[cfg(not(target_os = "linux"))]
    include_bytes!("../icons/tray/black-44.png")
}
pub fn create(app: &tauri::AppHandle, language: &Language) -> tauri::Result<()> {
    TrayIconBuilder::with_id("fontcatch")
        .icon(tauri::image::Image::from_bytes(icon())?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Fontcatch")
        .menu(&menu(app, language)?)
        .show_menu_on_left_click(cfg!(target_os = "linux"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app, None),
            "settings" => show(app, Some("settings")),
            "coffee" => {
                let _ = crate::open_link(app.clone(), "coffeeUrl".into());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    // On macOS the blur event can precede the click. Always open here;
                    // click-away and Escape provide predictable dismissal.
                    if window.is_focused().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        show(app, None);
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}
#[cfg(target_os = "macos")]
pub fn round_corners(app: &tauri::AppHandle) -> tauri::Result<()> {
    use objc2_app_kit::{NSColor, NSWindow};
    if let Some(window) = app.get_webview_window("main") {
        // setup runs on the main thread; Tauri owns this NSWindow for the entire call.
        let ns = unsafe { &*(window.ns_window()? as *const NSWindow) };
        ns.setOpaque(false);
        ns.setBackgroundColor(Some(&NSColor::clearColor()));
        if let Some(view) = ns.contentView() {
            view.setWantsLayer(true);
            if let Some(layer) = view.layer() {
                layer.setCornerRadius(14.0);
                layer.setMasksToBounds(true);
            }
        }
    }
    Ok(())
}
#[cfg(target_os = "windows")]
pub fn refresh_icon(app: &tauri::AppHandle) {
    set_pending(app, PENDING.load(std::sync::atomic::Ordering::Relaxed));
}

pub fn set_pending(app: &tauri::AppHandle, count: usize) {
    #[cfg(target_os = "windows")]
    PENDING.store(count, std::sync::atomic::Ordering::Relaxed);
    let Some(tray) = app.tray_by_id("fontcatch") else {
        return;
    };
    let _ = tray.set_tooltip(Some(if count == 0 {
        "Fontcatch".to_string()
    } else {
        format!("Fontcatch · {count}")
    }));
    #[cfg(target_os = "macos")]
    {
        // Keep the claw a template image; color only the separate native dot.
        let _ = tray.with_inner_tray_icon(move |inner| {
            use objc2::{AllocAnyThread, MainThreadMarker};
            use objc2_app_kit::{NSColor, NSForegroundColorAttributeName};
            use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            if let Some(button) = inner.ns_status_item().and_then(|item| item.button(mtm)) {
                let text = NSMutableAttributedString::initWithString(
                    NSMutableAttributedString::alloc(),
                    &NSString::from_str(if count > 0 { "●" } else { "" }),
                );
                if count > 0 {
                    let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                        211.0 / 255.0,
                        169.0 / 255.0,
                        1.0,
                        1.0,
                    );
                    // NSForegroundColorAttributeName requires NSColor, supplied here.
                    unsafe {
                        text.addAttribute_value_range(
                            NSForegroundColorAttributeName,
                            &color,
                            NSRange::new(0, 1),
                        );
                    }
                }
                button.setAttributedTitle(&text);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let image = if count > 0 {
            #[cfg(target_os = "windows")]
            {
                if icon() == include_bytes!("../icons/tray/black-44.png") {
                    include_bytes!("../icons/tray/black-pending-44.png").as_slice()
                } else {
                    include_bytes!("../icons/tray/white-pending-44.png").as_slice()
                }
            }
            #[cfg(target_os = "linux")]
            {
                include_bytes!("../icons/tray/white-pending-44.png").as_slice()
            }
        } else {
            icon()
        };
        if let Ok(image) = tauri::image::Image::from_bytes(image) {
            let _ = tray.set_icon(Some(image));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aligns_below_mac_tray() {
        assert_eq!(
            position(
                (800., 0., 24., 24.),
                (0., 24., 1440., 876.),
                (360., 480.),
                8.,
                true
            ),
            (632., 32.)
        );
    }
    #[test]
    fn clamps_to_right_screen_edge() {
        assert_eq!(
            position(
                (1420., 0., 20., 24.),
                (0., 24., 1440., 876.),
                (360., 480.),
                8.,
                true
            )
            .0,
            1080.
        );
    }
    #[test]
    fn opens_above_windows_taskbar() {
        assert_eq!(
            position(
                (1300., 860., 24., 40.),
                (0., 0., 1440., 860.),
                (360., 480.),
                8.,
                false
            )
            .1,
            372.
        );
    }
    #[test]
    fn handles_negative_monitor_origin_and_retina() {
        assert_eq!(
            position(
                (-30., 0., 28., 48.),
                (-2880., 48., 2880., 1752.),
                (720., 960.),
                16.,
                true
            ),
            (-720., 64.)
        );
    }
}
