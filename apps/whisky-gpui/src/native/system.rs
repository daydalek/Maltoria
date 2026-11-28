//! Host system services for the native backend: info, shell open/reveal, and
//! file picker panels.

use serde_json::{Value, json};

use crate::native::store::normalize_path;

pub(crate) fn system_info() -> Value {
    json!({
        "bundleVersion": env!("CARGO_PKG_VERSION"),
        "buildNumber": "1",
        "osVersion": os_version(),
    })
}

fn os_version() -> String {
    #[cfg(target_os = "windows")]
    {
        "Windows".to_owned()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux".to_owned()
    }
    #[cfg(target_os = "macos")]
    {
        "macOS".to_owned()
    }
}

fn open_path(path: &str) -> Result<Value, (String, String)> {
    let normalized = normalize_path(path);
    #[cfg(target_os = "windows")]
    {
        // `explorer` returns non-zero for some successful opens; `cmd /c start`
        // has shell-injection surface, so use explorer and accept its status.
        let result = std::process::Command::new("explorer")
            .arg(&normalized)
            .spawn();
        return match result {
            Ok(_) => Ok(json!({})),
            Err(error) => Err(("open_failed".into(), format!("Cannot open {normalized}: {error}"))),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let result = std::process::Command::new("open").arg(&normalized).status();
        return match result {
            Ok(status) if status.success() => Ok(json!({})),
            _ => Err(("open_failed".into(), format!("Cannot open {normalized}"))),
        };
    }
    #[cfg(target_os = "linux")]
    {
        let result = std::process::Command::new("xdg-open").arg(&normalized).status();
        return match result {
            Ok(status) if status.success() => Ok(json!({})),
            _ => Err(("open_failed".into(), format!("Cannot open {normalized}"))),
        };
    }
}

pub(crate) fn open(params: &Value) -> Result<Value, (String, String)> {
    let target = params
        .get("path")
        .or_else(|| params.get("url"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "system.open needs path".into()))?;
    if target.starts_with("http://") || target.starts_with("https://") {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", target])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|error| ("open_failed".into(), error.to_string()))?;
            return Ok(json!({}));
        }
        #[cfg(not(target_os = "windows"))]
        return open_path(target);
    }
    open_path(target)
}

pub(crate) fn reveal(params: &Value) -> Result<Value, (String, String)> {
    let target = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "system.reveal needs path".into()))?;
    let normalized = normalize_path(target);
    #[cfg(target_os = "windows")]
    {
        let select = format!("/select,{}", normalized.replace('/', "\\"));
        std::process::Command::new("explorer")
            .arg(select)
            .spawn()
            .map_err(|error| ("reveal_failed".into(), error.to_string()))?;
        return Ok(json!({}));
    }
    #[cfg(not(target_os = "windows"))]
    {
        return open_path(&normalized);
    }
}

/// `system.files`: existence/type probe batch for the UI.
pub(crate) fn files(params: &Value) -> Result<Value, (String, String)> {
    let paths = params
        .get("paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = paths
        .iter()
        .filter_map(Value::as_str)
        .map(|path| {
            let normalized = normalize_path(path);
            let meta = std::fs::metadata(&normalized).ok();
            json!({
                "path": normalized,
                "exists": meta.is_some(),
                "isFile": meta.as_ref().is_some_and(|m| m.is_file()),
                "isDirectory": meta.as_ref().is_some_and(|m| m.is_dir()),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "files": rows }))
}

/// File/folder panels. The native backend cannot block on a modal dialog
/// inside the bridge worker, so selection happens on a helper thread and the
/// reply is whatever the user picked; cancel maps to a null path.
pub(crate) fn choose_path(params: &Value) -> Result<Value, (String, String)> {
    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("open");
    let purpose = params
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    let folder_mode = mode.contains("folder") || mode == "directory";
    let picked = pick_path(folder_mode, start);
    Ok(json!({
        "purpose": purpose,
        "path": picked,
    }))
}

pub(crate) fn choose_folder(params: &Value) -> Result<Value, (String, String)> {
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    Ok(json!({
        "path": pick_path(true, start),
    }))
}

/// `system.chooseFolders`: modern multi-select folder picker reply.
pub(crate) fn choose_folders(params: &Value) -> Result<Value, (String, String)> {
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    Ok(json!({ "paths": pick_paths(true, start, true) }))
}

#[cfg(target_os = "windows")]
fn pick_path(folder: bool, start: &str) -> Option<String> {
    pick_paths(folder, start, false).into_iter().next()
}

#[cfg(target_os = "windows")]
fn pick_paths(folder: bool, start: &str, multiple: bool) -> Vec<String> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.is_err() {
        return Vec::new();
    }
    let paths = match pick_paths_com(folder, start, multiple) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("native picker failed: {error:?}");
            Vec::new()
        }
    };
    unsafe { CoUninitialize() };
    paths
}

#[cfg(target_os = "windows")]
fn pick_paths_com(
    folder: bool,
    start: &str,
    multiple: bool,
) -> windows::core::Result<Vec<String>> {
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
    use windows::Win32::UI::Shell::{
        FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
        IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
    };
    use windows::core::{HRESULT, HSTRING};

    let dialog: IFileOpenDialog = unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)? };
    let mut options = FOS_FILEMUSTEXIST;
    if multiple {
        options |= FOS_ALLOWMULTISELECT;
    }
    if folder {
        options |= FOS_PICKFOLDERS;
    }
    unsafe { dialog.SetOptions(options)? };

    let start = normalize_path(start);
    if !start.is_empty() && std::path::Path::new(&start).is_dir() {
        if let Ok(item) = unsafe {
            SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(start.as_str()), None)
        } {
            let _ = unsafe { dialog.SetFolder(&item) };
        }
    }

    if let Err(error) = unsafe { dialog.Show(HWND::default()) } {
        if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
            return Ok(Vec::new());
        }
        return Err(error);
    }

    let results = unsafe { dialog.GetResults()? };
    let count = unsafe { results.GetCount()? };
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        let item = unsafe { results.GetItemAt(index)? };
        let display = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
        let path = unsafe { display.to_string()? };
        unsafe { CoTaskMemFree(Some(display.0.cast())) };
        paths.push(normalize_path(&path));
    }
    Ok(paths)
}

#[cfg(not(target_os = "windows"))]
fn pick_path(_folder: bool, _start: &str) -> Option<String> {
    None
}
