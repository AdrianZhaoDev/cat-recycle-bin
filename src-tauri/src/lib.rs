mod bin_window;
mod controls;
mod recycle_bin;
mod settings;

use tauri::Manager;

#[derive(serde::Serialize)]
struct CursorPoint {
    x: f64,
    y: f64,
}

#[tauri::command]
fn cursor_position_local(window: tauri::WebviewWindow) -> Result<CursorPoint, String> {
    if window.label() != bin_window::BIN_LABEL {
        return Err("只有回收站窗口可以读取鼠标位置".into());
    }
    let cursor = window
        .cursor_position()
        .map_err(|error| error.to_string())?;
    let origin = window.outer_position().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    Ok(CursorPoint {
        x: (cursor.x - f64::from(origin.x)) / scale,
        y: (cursor.y - f64::from(origin.y)) / scale,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let initial = settings::load(app.handle());
            app.manage(settings::SettingsState(std::sync::Mutex::new(
                initial.clone(),
            )));
            app.manage(recycle_bin::start_worker(app.handle().clone()));
            bin_window::create(app, initial.scale_percent)?;
            controls::create(app, &initial)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != bin_window::BIN_LABEL {
                return;
            }
            match event {
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    bin_window::persist_position(window)
                }
                tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                    if window
                        .app_handle()
                        .state::<recycle_bin::DropQueue>()
                        .0
                        .send(paths.clone())
                        .is_err()
                    {
                        recycle_bin::emit_failure(window.app_handle(), "回收服务不可用".into());
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            bin_window::start_bin_drag,
            recycle_bin::open_recycle_bin,
            cursor_position_local,
            settings::get_settings,
            controls::show_bin_menu,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cat Recycle Bin");
}
