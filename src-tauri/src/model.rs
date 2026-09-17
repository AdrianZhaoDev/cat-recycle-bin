use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

const MAX_MODEL_BYTES: usize = 25 * 1024 * 1024;
const MODEL_FILE: &str = "custom-model.glb";

fn model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join(MODEL_FILE))
}

fn validate_glb(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 20 || bytes.len() > MAX_MODEL_BYTES {
        return Err("GLB 模型大小需在 20 字节至 25 MB 之间".into());
    }
    if &bytes[..4] != b"glTF" || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2 {
        return Err("请选择 glTF 2.0 格式的 .glb 文件".into());
    }
    if u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len() {
        return Err("GLB 文件长度不正确".into());
    }
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" || json_len > bytes.len() - 20 {
        return Err("GLB 缺少有效的 JSON 数据块".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len])
        .map_err(|_| "GLB 模型数据无效".to_string())?;
    for field in ["buffers", "images"] {
        if let Some(items) = json.get(field).and_then(serde_json::Value::as_array) {
            for item in items {
                if let Some(uri) = item.get("uri").and_then(serde_json::Value::as_str) {
                    if !uri.starts_with("data:") {
                        return Err("请使用已打包纹理和数据的独立 GLB 文件".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("无法读取模型：{error}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_MODEL_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("无法读取模型：{error}"))?;
    if bytes.len() > MAX_MODEL_BYTES {
        return Err("GLB 模型不能超过 25 MB".into());
    }
    Ok(bytes)
}

fn install(app: &AppHandle, source: &Path) -> Result<(), String> {
    if !source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"))
    {
        return Err("请选择 .glb 模型文件".into());
    }
    let bytes = read_bounded(source)?;
    validate_glb(&bytes)?;
    fs::write(model_path(app)?, bytes).map_err(|error| format!("无法保存模型：{error}"))?;
    let name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(80)
        .collect();
    let settings = crate::settings::set_model_name(app, Some(name))?;
    crate::controls::sync_checks(app, &settings);
    app.emit("bin://model-changed", ())
        .map_err(|error| error.to_string())
}

pub fn choose(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .file()
        .add_filter("glTF Binary (*.glb)", &["glb"])
        .pick_file(move |selected| {
            let Some(path) = selected.and_then(|value| value.as_path().map(Path::to_path_buf))
            else {
                return;
            };
            if let Err(error) = install(&handle, &path) {
                crate::recycle_bin::show_error(&error);
            }
        });
}

#[tauri::command]
pub fn read_active_model(app: AppHandle) -> Result<String, String> {
    if crate::settings::current(&app)?.model_name.is_none() {
        return Err("尚未选择自定义模型".into());
    }
    let bytes = read_bounded(&model_path(&app)?)?;
    validate_glb(&bytes)?;
    Ok(STANDARD.encode(bytes))
}

pub fn restore(app: &AppHandle) -> Result<(), String> {
    let settings = crate::settings::set_model_name(app, None)?;
    crate::controls::sync_checks(app, &settings);
    app.emit("bin://model-changed", ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn model_load_failed(app: AppHandle, reason: String) -> Result<(), String> {
    restore(&app)?;
    let reason: String = reason.chars().take(200).collect();
    crate::recycle_bin::show_error(&format!("自定义模型无法显示，已恢复默认模型。\n{reason}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_cat_is_a_self_contained_glb() {
        let bytes = include_bytes!("../../public/game/props/bin-cat.glb");
        validate_glb(bytes).unwrap();
        let example = include_bytes!("../../examples/color-cube.glb");
        validate_glb(example).unwrap();
    }

    #[test]
    fn rejects_non_glb_data() {
        assert!(validate_glb(b"not a glb").is_err());
    }
}
