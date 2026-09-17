use serde::Serialize;
use std::{
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::mpsc,
};
use tauri::{AppHandle, Emitter, Manager};
use windows::{
    Win32::{
        Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW},
        System::{
            Com::{
                CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            WindowsProgramming::DRIVE_FIXED,
        },
        UI::{
            Shell::{
                FILEOPERATION_FLAGS, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
                FOF_WANTNUKEWARNING, FOFX_ADDUNDORECORD, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE,
                FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
            },
            WindowsAndMessaging::{
                IDYES, MB_ICONERROR, MB_ICONQUESTION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
                MB_YESNO, MessageBoxW,
            },
        },
    },
    core::{PCWSTR, w},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancelled: Option<bool>,
}

impl BinResult {
    fn success() -> Self {
        Self {
            ok: true,
            error: None,
            cancelled: None,
        }
    }
    fn failure(error: String) -> Self {
        Self {
            ok: false,
            error: Some(error),
            cancelled: None,
        }
    }
    fn cancelled() -> Self {
        Self {
            ok: false,
            error: None,
            cancelled: Some(true),
        }
    }
}

pub struct DropQueue(pub mpsc::Sender<Vec<PathBuf>>);

pub fn start_worker(app: AppHandle) -> DropQueue {
    let (sender, receiver) = mpsc::channel::<Vec<PathBuf>>();
    std::thread::spawn(move || {
        // One native worker serializes confirmation dialogs and shell operations.
        for paths in receiver {
            let result = process_drop(&app, paths);
            if let Some(error) = &result.error {
                show_error(error);
            }
            let _ = app.emit("bin://result", result);
        }
    });
    DropQueue(sender)
}

pub fn emit_failure(app: &AppHandle, error: String) {
    let _ = app.emit("bin://result", BinResult::failure(error));
}

fn process_drop(app: &AppHandle, paths: Vec<PathBuf>) -> BinResult {
    if let Err(error) = validate_paths(&paths, &protected_paths(app)) {
        return BinResult::failure(error);
    }
    let settings = match crate::settings::current(app) {
        Ok(settings) => settings,
        Err(error) => return BinResult::failure(error),
    };
    if settings.confirm_delete && !confirm_drop(&paths) {
        return BinResult::cancelled();
    }
    match recycle_paths_sta(&paths) {
        Ok(()) => BinResult::success(),
        Err(error) => BinResult::failure(error),
    }
}

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn protected_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        paths.push(executable);
    }
    if let Ok(config_dir) = app.path().app_config_dir() {
        paths.push(config_dir);
    }
    if let Ok(data_dir) = app.path().app_data_dir() {
        paths.push(data_dir);
    }
    if let Ok(local_dir) = app.path().app_local_data_dir() {
        paths.push(local_dir);
    }
    paths
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn contains_path(parent: &Path, child: &Path) -> bool {
    let parent = normalized(parent);
    let child = normalized(child);
    child == parent || child.starts_with(&(parent.trim_end_matches('\\').to_owned() + "\\"))
}

fn validate_paths(paths: &[PathBuf], protected: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有收到可回收的文件或文件夹".into());
    }
    let mut resolved: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(format!("不能回收磁盘根目录或无效路径：{}", path.display()));
        }
        fs::symlink_metadata(path).map_err(|error| format!("{}：{error}", path.display()))?;
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("{}：{error}", path.display()))?;
        let normalized_path = normalized(&canonical);
        if normalized_path.contains("\\$recycle.bin\\")
            || normalized_path.ends_with("\\$recycle.bin")
        {
            return Err("不能回收 Windows 回收站中的项目".into());
        }
        for own in protected {
            if let Ok(own) = own.canonicalize() {
                if contains_path(&canonical, &own)
                    || (own.is_dir() && contains_path(&own, &canonical))
                {
                    return Err(format!("不能回收程序文件或设置数据：{}", path.display()));
                }
            }
        }
        if resolved
            .iter()
            .any(|other| contains_path(other, &canonical) || contains_path(&canonical, other))
        {
            return Err("拖入的项目存在重复或父子目录重叠".into());
        }
        resolved.push(canonical);
        // The Windows Recycle Bin is not available on network and removable volumes.
        // Refuse those sources instead of letting Shell fall back to permanent deletion.
        let encoded = wide(path.as_os_str());
        let mut volume = [0u16; 32768];
        unsafe {
            GetVolumePathNameW(PCWSTR(encoded.as_ptr()), &mut volume)
                .map_err(|error| format!("无法识别所在磁盘 {}：{error}", path.display()))?;
            if GetDriveTypeW(PCWSTR(volume.as_ptr())) != DRIVE_FIXED {
                return Err(format!(
                    "此位置不支持安全移入 Windows 回收站：{}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn confirm_drop(paths: &[PathBuf]) -> bool {
    let text = if paths.len() == 1 {
        format!(
            "是否将“{}”移入 Windows 回收站？",
            paths[0].file_name().unwrap_or_default().to_string_lossy()
        )
    } else {
        format!("是否将这 {} 个项目移入 Windows 回收站？", paths.len())
    };
    let encoded = wide(std::ffi::OsStr::new(&text));
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(encoded.as_ptr()),
            w!("猫咪回收站"),
            MB_YESNO | MB_ICONQUESTION | MB_TOPMOST | MB_SETFOREGROUND,
        ) == IDYES
    }
}

pub fn show_error(message: &str) {
    let text = format!("未能移入 Windows 回收站。\n\n{message}");
    let encoded = wide(std::ffi::OsStr::new(&text));
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(encoded.as_ptr()),
            w!("猫咪回收站"),
            MB_OK | MB_ICONERROR | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

fn recycle_paths_sta(paths: &[PathBuf]) -> Result<(), String> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| error.to_string())?;
    }
    let result = recycle_paths_initialized(paths);
    unsafe {
        CoUninitialize();
    }
    result
}

fn recycle_paths_initialized(paths: &[PathBuf]) -> Result<(), String> {
    unsafe {
        let operation: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
            .map_err(|error| error.to_string())?;
        let flags = FILEOPERATION_FLAGS(
            FOF_ALLOWUNDO.0
                | FOFX_RECYCLEONDELETE.0
                | FOFX_ADDUNDORECORD.0
                | FOF_NOCONFIRMATION.0
                | FOF_WANTNUKEWARNING.0
                | FOF_NOERRORUI.0
                | FOFX_EARLYFAILURE.0
                | FOF_SILENT.0,
        );
        operation
            .SetOperationFlags(flags)
            .map_err(|error| error.to_string())?;
        for path in paths {
            let encoded = wide(path.as_os_str());
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(encoded.as_ptr()), None)
                .map_err(|error| format!("{}：{error}", path.display()))?;
            operation
                .DeleteItem(&item, None)
                .map_err(|error| format!("{}：{error}", path.display()))?;
        }
        operation
            .PerformOperations()
            .map_err(|error| error.to_string())?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(|error| error.to_string())?
            .as_bool()
        {
            return Err("Windows 中止了部分或全部回收操作".into());
        }
        if paths.iter().any(|path| path.exists()) {
            return Err("至少一个项目仍在原位置，回收操作未完成".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_recycle_bin() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer.exe")
        .arg("shell:RecycleBinFolder")
        .creation_flags(0x08000000)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_drive_root() {
        assert!(validate_paths(&[PathBuf::from("C:\\")], &[]).is_err());
    }

    #[test]
    fn path_guard_recognizes_ancestors_without_matching_sibling_names() {
        let own = PathBuf::from("C:\\Apps\\Cat Bin\\cat-recycle-bin.exe");
        assert!(contains_path(&PathBuf::from("C:\\Apps\\Cat Bin"), &own));
        assert!(!contains_path(&PathBuf::from("C:\\Apps\\Cat"), &own));
        assert!(!contains_path(&PathBuf::from("C:\\Apps\\Cat Bin 2"), &own));
    }

    #[test]
    fn rejects_protected_data_and_overlapping_drop_paths() {
        let root = std::env::temp_dir().join(format!(
            "cat-recycle-bin-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let child = root.join("child.txt");
        fs::write(&child, b"guard test").unwrap();
        assert!(validate_paths(std::slice::from_ref(&child), std::slice::from_ref(&root)).is_err());
        assert!(validate_paths(&[root.clone(), child.clone()], &[]).is_err());
        let _ = fs::remove_file(child);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn moves_our_own_temporary_file_to_windows_recycle_bin() {
        let name = format!(
            "cat-recycle-bin-test-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        fs::write(&path, b"Cat Recycle Bin native integration test").unwrap();
        validate_paths(std::slice::from_ref(&path), &[]).unwrap();
        let result = recycle_paths_sta(std::slice::from_ref(&path));
        if result.is_err() {
            let _ = fs::remove_file(&path);
        }
        result.unwrap();
        assert!(!path.exists());
    }
}
