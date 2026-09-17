use tauri::{
    AppHandle, Manager, Wry,
    menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub struct Controls {
    menu: Menu<Wry>,
    confirm: CheckMenuItem<Wry>,
    sound: CheckMenuItem<Wry>,
    size_100: CheckMenuItem<Wry>,
    size_150: CheckMenuItem<Wry>,
    size_200: CheckMenuItem<Wry>,
    size_300: CheckMenuItem<Wry>,
    restore_model: MenuItem<Wry>,
}

pub fn create(app: &tauri::App, settings: &crate::settings::Settings) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开 Windows 回收站", true, None::<&str>)?;
    let confirm = CheckMenuItem::with_id(
        app,
        "confirm",
        "删除前提醒",
        true,
        settings.confirm_delete,
        None::<&str>,
    )?;
    let sound = CheckMenuItem::with_id(
        app,
        "sound",
        "播放音效",
        true,
        settings.sound_enabled,
        None::<&str>,
    )?;
    let size_100 = CheckMenuItem::with_id(
        app,
        "size-100",
        "尺寸 100%（原尺寸）",
        true,
        settings.scale_percent == 100,
        None::<&str>,
    )?;
    let size_150 = CheckMenuItem::with_id(
        app,
        "size-150",
        "尺寸 150%",
        true,
        settings.scale_percent == 150,
        None::<&str>,
    )?;
    let size_200 = CheckMenuItem::with_id(
        app,
        "size-200",
        "尺寸 200%（更清晰）",
        true,
        settings.scale_percent == 200,
        None::<&str>,
    )?;
    let size_300 = CheckMenuItem::with_id(
        app,
        "size-300",
        "尺寸 300%（展示）",
        true,
        settings.scale_percent == 300,
        None::<&str>,
    )?;
    let choose_model = MenuItem::with_id(
        app,
        "choose-model",
        "更换 3D 模型（GLB）…",
        true,
        None::<&str>,
    )?;
    let restore_model = MenuItem::with_id(
        app,
        "restore-model",
        "恢复默认猫咪模型",
        settings.model_name.is_some(),
        None::<&str>,
    )?;
    let exit = MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?;
    let separator_1 = PredefinedMenuItem::separator(app)?;
    let separator_2 = PredefinedMenuItem::separator(app)?;
    let separator_3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &confirm,
            &sound,
            &separator_1,
            &size_100,
            &size_150,
            &size_200,
            &size_300,
            &separator_2,
            &choose_model,
            &restore_model,
            &separator_3,
            &exit,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id("cat-recycle-bin")
        .menu(&menu)
        .tooltip("猫咪回收站")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                if let Err(error) = crate::recycle_bin::open_recycle_bin() {
                    crate::recycle_bin::show_error(&error);
                }
            }
            "confirm" => match crate::settings::toggle_confirm(app) {
                Ok(settings) => sync_checks(app, &settings),
                Err(error) => crate::recycle_bin::show_error(&error),
            },
            "sound" => match crate::settings::toggle_sound(app) {
                Ok(settings) => sync_checks(app, &settings),
                Err(error) => crate::recycle_bin::show_error(&error),
            },
            "size-100" => set_size(app, 100),
            "size-150" => set_size(app, 150),
            "size-200" => set_size(app, 200),
            "size-300" => set_size(app, 300),
            "choose-model" => crate::model::choose(app),
            "restore-model" => {
                if let Err(error) = crate::model::restore(app) {
                    crate::recycle_bin::show_error(&error);
                } else if let Ok(settings) = crate::settings::current(app) {
                    sync_checks(app, &settings);
                }
            }
            "exit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    app.manage(Controls {
        menu,
        confirm,
        sound,
        size_100,
        size_150,
        size_200,
        size_300,
        restore_model,
    });
    Ok(())
}

pub(crate) fn sync_checks(app: &AppHandle, settings: &crate::settings::Settings) {
    let controls = app.state::<Controls>();
    let _ = controls.confirm.set_checked(settings.confirm_delete);
    let _ = controls.sound.set_checked(settings.sound_enabled);
    let _ = controls.size_100.set_checked(settings.scale_percent == 100);
    let _ = controls.size_150.set_checked(settings.scale_percent == 150);
    let _ = controls.size_200.set_checked(settings.scale_percent == 200);
    let _ = controls.size_300.set_checked(settings.scale_percent == 300);
    let _ = controls
        .restore_model
        .set_enabled(settings.model_name.is_some());
}

fn set_size(app: &AppHandle, value: u16) {
    let result = (|| {
        let before = crate::settings::current(app)?;
        crate::bin_window::resize(app, value)?;
        match crate::settings::set_scale(app, value) {
            Ok(settings) => {
                sync_checks(app, &settings);
                Ok(())
            }
            Err(error) => {
                let _ = crate::bin_window::resize(app, before.scale_percent);
                Err(error)
            }
        }
    })();
    if let Err(error) = result {
        if let Ok(settings) = crate::settings::current(app) {
            sync_checks(app, &settings);
        }
        crate::recycle_bin::show_error(&error);
    }
}

#[tauri::command]
pub fn show_bin_menu(window: tauri::Window) -> Result<(), String> {
    if window.label() != crate::bin_window::BIN_LABEL {
        return Err("只有回收站窗口可以打开此菜单".into());
    }
    window
        .set_focusable(true)
        .map_err(|error| error.to_string())?;
    let _ = window.set_focus();
    let result = window
        .state::<Controls>()
        .menu
        .popup(window.clone())
        .map_err(|error| error.to_string());
    let _ = window.set_focusable(false);
    result
}
