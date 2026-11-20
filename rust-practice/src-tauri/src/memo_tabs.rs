use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{
    Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const DETACHED_PREFIX: &str = "memo-tab-";

#[derive(Clone, Copy, Deserialize)]
pub struct TabBarBounds {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl TabBarBounds {
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.left + self.width && y >= self.top && y < self.top + self.height
    }
}

#[derive(Default)]
struct DropState {
    bounds: Option<TabBarBounds>,
    preview: bool,
}

#[derive(Default)]
pub struct MemoDropTarget(Mutex<DropState>);

fn set_drop_preview(window: &WebviewWindow, target: &MemoDropTarget, preview: bool) {
    let changed = target.0.lock().is_ok_and(|mut state| {
        if state.preview == preview {
            return false;
        }
        state.preview = preview;
        true
    });
    if changed && let Err(error) = window.emit_to("memo", "daymark-memo-drop-preview", preview) {
        log::warn!("메모 탭 놓기 안내 실패: {error}");
    }
}

fn docking_position(
    window: &WebviewWindow,
    memo: &WebviewWindow,
    bar_bottom: i32,
) -> tauri::Result<PhysicalPosition<i32>> {
    let parent = memo.outer_position()?;
    let parent_size = memo.outer_size()?;
    let size = window.outer_size()?;
    let gap = (8.0 * memo.scale_factor()?).round() as i32;
    let right = parent.x + parent_size.width as i32 + gap;
    let left = parent.x - size.width as i32 - gap;
    let below = parent.y + parent_size.height as i32 + gap;
    if let Some(monitor) = memo.current_monitor()? {
        let area = monitor.work_area();
        let max_x = area.position.x + area.size.width.saturating_sub(size.width) as i32;
        let max_y = area.position.y + area.size.height.saturating_sub(size.height) as i32;
        let y = parent.y.clamp(area.position.y, max_y);
        if right <= max_x {
            return Ok(PhysicalPosition::new(right, y));
        }
        if left >= area.position.x {
            return Ok(PhysicalPosition::new(left, y));
        }
        let x = parent.x.clamp(area.position.x, max_x);
        return Ok(PhysicalPosition::new(
            x,
            if below <= max_y {
                below
            } else {
                bar_bottom + gap
            },
        ));
    }
    Ok(PhysicalPosition::new(right, parent.y))
}

#[tauri::command]
pub fn clear_memo_drop_preview(window: WebviewWindow, target: State<'_, MemoDropTarget>) {
    if window.label().starts_with(DETACHED_PREFIX) {
        set_drop_preview(&window, &target, false);
    }
}

#[tauri::command]
pub fn register_memo_tab_bar(
    window: WebviewWindow,
    target: State<'_, MemoDropTarget>,
    bounds: TabBarBounds,
) -> Result<(), String> {
    if window.label() != "memo" {
        return Err("메모 창에서만 탭바를 등록할 수 있습니다.".into());
    }
    let mut state = target.0.lock().map_err(|error| error.to_string())?;
    state.bounds = Some(bounds);
    Ok(())
}

#[tauri::command]
pub fn begin_memo_tab_drag(window: WebviewWindow) -> Result<[i32; 2], String> {
    if !window.label().starts_with(DETACHED_PREFIX) {
        return Err("분리된 메모만 드래그할 수 있습니다.".into());
    }
    let cursor = window
        .cursor_position()
        .map_err(|error| error.to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    Ok([cursor.x as i32 - position.x, cursor.y as i32 - position.y])
}

#[tauri::command]
pub fn place_memo_tab(
    window: WebviewWindow,
    target: State<'_, MemoDropTarget>,
    offset: [i32; 2],
    drop_position: Option<[f64; 2]>,
) -> Result<(), String> {
    if !window.label().starts_with(DETACHED_PREFIX) {
        return Err("분리된 메모만 옮길 수 있습니다.".into());
    }
    let result = (|| -> tauri::Result<()> {
        let cursor = match drop_position {
            Some([x, y]) => PhysicalPosition::new(x, y),
            None => window.cursor_position()?,
        };
        let mut hovered = None;
        if let Some(memo) = window.get_webview_window("memo")
            && memo.is_visible()?
            && !memo.is_minimized()?
        {
            let bounds = target.0.lock().ok().and_then(|state| state.bounds);
            if let Some(bounds) = bounds {
                let origin = memo.inner_position()?;
                let scale = memo.scale_factor()?;
                let x = (cursor.x - f64::from(origin.x)) / scale;
                let y = (cursor.y - f64::from(origin.y)) / scale;
                if bounds.contains(x, y) {
                    let bottom = origin.y + ((bounds.top + bounds.height) * scale).ceil() as i32;
                    hovered = Some((memo, bottom));
                }
            }
        }
        set_drop_preview(
            &window,
            &target,
            drop_position.is_none() && hovered.is_some(),
        );
        if let Some((memo, bottom)) = hovered {
            if drop_position.is_some() {
                window.close()?;
                return memo.set_focus();
            }
            let position = docking_position(&window, &memo, bottom)?;
            if window.outer_position()? != position {
                window.set_position(position)?;
            }
            return Ok(());
        }
        window.set_position(PhysicalPosition::new(
            (cursor.x as i32).saturating_sub(offset[0]),
            (cursor.y as i32).saturating_sub(offset[1]),
        ))
    })();
    result.map_err(|error| error.to_string())
}

#[derive(Deserialize, Serialize)]
struct Tab {
    id: String,
    title: String,
    html: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Tabs {
    tabs: Vec<Tab>,
    active_id: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TabEdit {
    Add { id: String, title: String },
    Select { id: String },
    Rename { id: String, title: String },
    Content { id: String, html: String },
    Delete { id: String },
}

fn tabs_path(window: &WebviewWindow) -> tauri::Result<PathBuf> {
    Ok(window.path().app_local_data_dir()?.join("memo-tabs.json"))
}

#[tauri::command]
pub fn edit_memo_tab(window: WebviewWindow, edit: TabEdit) -> Result<(), String> {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let path = tabs_path(&window)?;
        let mut data: Tabs = serde_json::from_slice(&fs::read(&path)?)?;
        match edit {
            TabEdit::Add { id, title } => {
                data.active_id = id.clone();
                data.tabs.push(Tab {
                    id,
                    title,
                    html: String::new(),
                });
            }
            TabEdit::Select { id } => data.active_id = id,
            TabEdit::Delete { id } => {
                if data.tabs.len() <= 1 {
                    return Err("마지막 메모는 삭제할 수 없습니다.".into());
                }
                data.tabs.retain(|tab| tab.id != id);
                if data.active_id == id {
                    data.active_id.clone_from(&data.tabs[0].id);
                }
            }
            TabEdit::Rename { id, title } => {
                data.tabs
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .ok_or("메모를 찾을 수 없습니다.")?
                    .title = title;
            }
            TabEdit::Content { id, html } => {
                data.tabs
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .ok_or("메모를 찾을 수 없습니다.")?
                    .html = html;
            }
        }
        crate::documents::write_data(&path, &serde_json::to_string(&data)?)?;
        Ok(())
    })();
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn detached_memo_ids(window: WebviewWindow) -> Vec<String> {
    window
        .webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix(DETACHED_PREFIX).map(str::to_owned))
        .collect()
}

#[tauri::command]
pub async fn detach_memo_tab(window: WebviewWindow, id: String) -> Result<(), String> {
    if window.label() != "memo"
        || id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("분리할 메모 탭을 확인할 수 없습니다.".into());
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let label = format!("{DETACHED_PREFIX}{id}");
        if let Some(popup) = window.get_webview_window(&label) {
            popup.set_focus()?;
            return Ok(());
        }
        let data: Tabs = serde_json::from_slice(&fs::read(tabs_path(&window)?)?)?;
        let entry = data
            .tabs
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("메모를 찾을 수 없습니다.")?;
        let cursor = window.cursor_position()?;
        let popup = WebviewWindowBuilder::new(&window, label, WebviewUrl::App("memo.html".into()))
            .title(&entry.title)
            .inner_size(340.0, 400.0)
            .min_inner_size(240.0, 220.0)
            .decorations(false)
            .visible(false)
            .build()?;
        let mut x = cursor.x as i32 - 42;
        let mut y = cursor.y as i32 - 16;
        if let Some(monitor) = popup.monitor_from_point(cursor.x, cursor.y)? {
            let area = monitor.work_area();
            let size = popup.outer_size()?;
            x = x.clamp(
                area.position.x,
                area.position.x + area.size.width.saturating_sub(size.width) as i32,
            );
            y = y.clamp(
                area.position.y,
                area.position.y + area.size.height.saturating_sub(size.height) as i32,
            );
        }
        popup.set_position(PhysicalPosition::new(x, y))?;
        popup.show()?;
        popup.set_focus()?;
        Ok(())
    })();
    result.map_err(|error| error.to_string())
}

pub fn notify_reattached(window: &tauri::Window) {
    let id = window.label().strip_prefix(DETACHED_PREFIX);
    if let Err(error) = window.emit_to("memo", "daymark-memo-reattached", id) {
        log::warn!("메모 탭 복귀 알림 실패: {error}");
    }
}
