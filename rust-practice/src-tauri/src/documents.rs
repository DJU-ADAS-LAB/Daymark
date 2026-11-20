use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::{fs, io, path::PathBuf};
use tauri::{Emitter, Manager, WebviewWindow};

#[cfg(windows)]
pub fn migrate_legacy_data(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let current = app.path().app_local_data_dir()?;
    let marker = current.join(".legacy-data-imported");
    if marker.try_exists()? {
        return Ok(());
    }
    let Some(parent) = current.parent() else {
        return Ok(());
    };
    let legacy = parent.join("com.tauri.dev");
    if legacy == current {
        return Ok(());
    }
    fs::create_dir_all(&current)?;
    for name in [
        "tasks.json",
        "memo.html",
        "window-main.json",
        "window-checklist.json",
        "window-memo.json",
        "popup-checklist.open",
        "popup-memo.open",
    ] {
        let source = legacy.join(name);
        let target = current.join(name);
        if source.is_file() && !target.try_exists()? {
            fs::copy(source, target)?;
        }
    }
    fs::write(marker, b"")?;
    Ok(())
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Document {
    Tasks,
    Memo,
    MemoTabs,
}

fn data_path(window: &WebviewWindow, document: Document) -> Result<PathBuf, tauri::Error> {
    let name = match document {
        Document::Tasks => "tasks.json",
        Document::Memo => "memo.html",
        Document::MemoTabs => "memo-tabs.json",
    };
    Ok(window.path().app_local_data_dir()?.join(name))
}

pub(crate) fn write_data(path: &std::path::Path, data: &str) -> io::Result<()> {
    fs::create_dir_all(path.parent().expect("document data directory"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, data)?;
    fs::rename(temporary, path)
}

fn migrate_schedules(
    path: &std::path::Path,
    legacy_tasks: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = path.with_file_name("schedules.json");
    let data = match fs::read(&source) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let schedules: Vec<Value> = serde_json::from_slice(&data)?;
    let mut tasks: Vec<Value> = match fs::read_to_string(path) {
        Ok(data) => serde_json::from_str(&data)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            serde_json::from_str(legacy_tasks.unwrap_or("[]"))?
        }
        Err(error) => return Err(error.into()),
    };
    let mut ids: HashSet<String> = tasks
        .iter()
        .filter_map(|task| task["id"].as_str().map(str::to_owned))
        .collect();
    for schedule in schedules {
        let id = schedule["id"].as_str().ok_or("일정 ID 누락")?;
        let title = schedule["title"].as_str().ok_or("일정 이름 누락")?;
        let start = schedule["start"].as_i64().ok_or("일정 시작 시간 누락")?;
        let end = schedule["end"].as_i64().ok_or("일정 종료 시간 누락")?;
        let lead = schedule["lead"].as_i64().ok_or("일정 알림 시간 누락")?;
        if ids.insert(id.to_owned()) {
            tasks.push(json!({ "id": id, "text": title, "done": false, "start": start, "end": end, "lead": lead }));
        }
    }
    write_data(path, &serde_json::to_string(&tasks)?)?;
    fs::rename(&source, source.with_file_name("schedules.migrated.json"))?;
    Ok(())
}

#[tauri::command]
pub fn load_document(
    window: WebviewWindow,
    document: Document,
    legacy: Option<String>,
) -> Result<Option<String>, String> {
    let result = (|| -> Result<Option<String>, Box<dyn std::error::Error>> {
        let path = data_path(&window, document)?;
        if matches!(document, Document::Tasks) {
            migrate_schedules(&path, legacy.as_deref())?;
        }
        match fs::read_to_string(&path) {
            Ok(data) => Ok(Some(data)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if let Some(data) = legacy.as_deref() {
                    write_data(&path, data)?;
                }
                Ok(legacy)
            }
            Err(error) => Err(error.into()),
        }
    })();
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_document(
    window: WebviewWindow,
    document: Document,
    data: String,
) -> Result<(), String> {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        write_data(&data_path(&window, document)?, &data)?;
        if matches!(document, Document::Tasks) {
            let other = if window.label() == "main" {
                "checklist"
            } else {
                "main"
            };
            if let Err(error) = window.emit_to(other, "daymark-tasks-changed", ()) {
                log::warn!("할 일 변경 알림 실패: {error}");
            }
        }
        Ok(())
    })();
    result.map_err(|error| error.to_string())
}
