use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const BIN_LABEL: &str = "trash-bin";
const WIDTH_DIP: f64 = 76.0;
const HEIGHT_DIP: f64 = 88.0;
const MARGIN_DIP: f64 = 16.0;

fn logical_size(scale_percent: u16) -> (f64, f64) {
    let factor = f64::from(scale_percent) / 100.0;
    (WIDTH_DIP * factor, HEIGHT_DIP * factor)
}

#[derive(Serialize, Deserialize)]
struct SavedPosition {
    x: i32,
    y: i32,
}

fn position_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("bin-position.json"))
}

pub fn create(app: &tauri::App, scale_percent: u16) -> tauri::Result<WebviewWindow> {
    let (width_dip, height_dip) = logical_size(scale_percent);
    let window = WebviewWindowBuilder::new(app, BIN_LABEL, WebviewUrl::App("index.html".into()))
        .title("猫咪回收站")
        .additional_browser_args("--autoplay-policy=no-user-gesture-required")
        .data_directory(app.path().app_local_data_dir()?.join("bin-webview"))
        .inner_size(width_dip, height_dip)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .shadow(false)
        .resizable(false)
        .visible(true)
        .build()?;

    let primary = window
        .primary_monitor()?
        .or(window.current_monitor()?)
        .ok_or(tauri::Error::WindowNotFound)?;
    let area = primary.work_area();
    let scale = primary.scale_factor();
    let width = (width_dip * scale).round() as i32;
    let height = (height_dip * scale).round() as i32;
    let margin = (MARGIN_DIP * scale).round() as i32;
    let fallback = PhysicalPosition::new(
        area.position.x + area.size.width as i32 - width - margin,
        area.position.y + area.size.height as i32 - height - margin,
    );
    let saved = position_path(app.handle())
        .ok()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<SavedPosition>(&bytes).ok())
        .map(|point| PhysicalPosition::new(point.x, point.y));
    let position = saved
        .filter(|point| {
            window.available_monitors().ok().is_some_and(|monitors| {
                monitors.iter().any(|monitor| {
                    let area = monitor.work_area();
                    let width = (width_dip * monitor.scale_factor()).round() as i32;
                    let height = (height_dip * monitor.scale_factor()).round() as i32;
                    point.x >= area.position.x
                        && point.y >= area.position.y
                        && point.x + width <= area.position.x + area.size.width as i32
                        && point.y + height <= area.position.y + area.size.height as i32
                })
            })
        })
        .unwrap_or(fallback);
    window.set_position(position)?;
    window.set_size(PhysicalSize::new(width as u32, height as u32))?;
    Ok(window)
}

pub fn resize(app: &tauri::AppHandle, scale_percent: u16) -> Result<(), String> {
    if !crate::settings::valid_scale(scale_percent) {
        return Err("不支持的回收站尺寸".into());
    }
    let window = app
        .get_webview_window(BIN_LABEL)
        .ok_or("回收站窗口不可用")?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or("找不到显示器")?;
    let factor = window.scale_factor().map_err(|error| error.to_string())?;
    let (width_dip, height_dip) = logical_size(scale_percent);
    let width = (width_dip * factor).round() as i32;
    let height = (height_dip * factor).round() as i32;
    let old_size = window.outer_size().map_err(|error| error.to_string())?;
    let old_position = window.outer_position().map_err(|error| error.to_string())?;
    let area = monitor.work_area();
    let min_x = area.position.x;
    let min_y = area.position.y;
    let max_x = (area.position.x + area.size.width as i32 - width).max(min_x);
    let max_y = (area.position.y + area.size.height as i32 - height).max(min_y);
    let x = (old_position.x + old_size.width as i32 - width).clamp(min_x, max_x);
    let y = (old_position.y + old_size.height as i32 - height).clamp(min_y, max_y);
    window
        .set_size(PhysicalSize::new(width as u32, height as u32))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    write_position(app, PhysicalPosition::new(x, y));
    Ok(())
}

fn write_position(app: &tauri::AppHandle, position: PhysicalPosition<i32>) {
    let Ok(path) = position_path(app) else {
        return;
    };
    if let Ok(bytes) = serde_json::to_vec(&SavedPosition {
        x: position.x,
        y: position.y,
    }) {
        let _ = fs::write(path, bytes);
    }
}

pub fn persist_position(window: &tauri::Window) {
    if window.label() != BIN_LABEL {
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    write_position(window.app_handle(), position);
}

#[tauri::command]
pub fn start_bin_drag(window: WebviewWindow) -> Result<(), String> {
    if window.label() != BIN_LABEL {
        return Err("只有回收站窗口可以拖动".into());
    }
    window.start_dragging().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_offers_original_and_clearer_sizes() {
        assert_eq!(logical_size(100), (76.0, 88.0));
        assert_eq!(logical_size(200), (152.0, 176.0));
        assert_eq!(logical_size(300), (228.0, 264.0));
    }
}
