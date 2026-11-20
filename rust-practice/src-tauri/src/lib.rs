use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc::{self, RecvTimeoutError, Sender},
    time::Duration,
};
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

mod documents;
mod memo_tabs;
mod reminders;

const CHECKLIST_WINDOW: &str = "checklist";
const MEMO_WINDOW: &str = "memo";
const POPUPS: [&str; 2] = [CHECKLIST_WINDOW, MEMO_WINDOW];

#[derive(Default)]
struct SessionClosing(AtomicBool);

struct CalendarLayoutSave(Sender<WebviewWindow>);

impl CalendarLayoutSave {
    fn start() -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<WebviewWindow>();
        std::thread::Builder::new()
            .name("daymark-calendar-layout".into())
            .spawn(move || {
                while let Ok(mut calendar) = receiver.recv() {
                    loop {
                        match receiver.recv_timeout(Duration::from_millis(250)) {
                            Ok(window) => calendar = window,
                            Err(RecvTimeoutError::Disconnected) => return,
                            Err(RecvTimeoutError::Timeout) => break,
                        }
                    }
                    let handle = calendar.app_handle().clone();
                    let _ = calendar.run_on_main_thread(move || {
                        if let Some(calendar) = handle.get_webview_window("main") {
                            if let Err(error) = save_layout(&calendar) {
                                log::warn!("달력 창 상태 저장 실패: {error}");
                            }
                        }
                    });
                }
            })?;
        Ok(Self(sender))
    }
}

fn popup_state_path(window: &WebviewWindow, label: &str) -> tauri::Result<PathBuf> {
    Ok(window
        .path()
        .app_local_data_dir()?
        .join(format!("popup-{label}.open")))
}

fn remember_popup(window: &WebviewWindow, label: &str, open: bool) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let path = popup_state_path(window, label)?;
        if open {
            fs::create_dir_all(path.parent().expect("popup state directory"))?;
            fs::write(path, b"open")?;
        } else if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.into());
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        log::warn!("{label} 팝업 상태 저장 실패: {error}");
    }
}

#[derive(Deserialize, Serialize)]
struct WindowLayout {
    x: i32,
    y: i32,
    width: f64,
    height: f64,
    pinned: bool,
}

fn layout_path(window: &WebviewWindow) -> tauri::Result<PathBuf> {
    Ok(window
        .path()
        .app_local_data_dir()?
        .join(format!("window-{}.json", window.label())))
}

fn restore_layout(window: &WebviewWindow) -> Result<bool, Box<dyn std::error::Error>> {
    let data = match fs::read(layout_path(window)?) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let layout: WindowLayout = serde_json::from_slice(&data)?;
    let monitor = window.available_monitors()?.into_iter().find(|monitor| {
        let area = monitor.work_area();
        (area.position.x..area.position.x + area.size.width as i32).contains(&layout.x)
            && (area.position.y..area.position.y + area.size.height as i32).contains(&layout.y)
    });
    let was_on_connected_monitor = monitor.is_some();
    if let Some(monitor) = monitor.or(window.primary_monitor()?) {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let width = (layout.width * scale)
            .round()
            .clamp(1.0, area.size.width.max(1) as f64) as u32;
        let height = (layout.height * scale)
            .round()
            .clamp(1.0, area.size.height.max(1) as f64) as u32;
        let (x, y) = if was_on_connected_monitor {
            (layout.x, layout.y)
        } else {
            (
                layout.x.clamp(
                    area.position.x,
                    area.position.x + area.size.width.saturating_sub(width) as i32,
                ),
                layout.y.clamp(
                    area.position.y,
                    area.position.y + area.size.height.saturating_sub(height) as i32,
                ),
            )
        };
        window.set_position(PhysicalPosition::new(x, y))?;
        window.set_size(PhysicalSize::new(width, height))?;
    }
    window.set_always_on_top(layout.pinned)?;
    Ok(true)
}

fn save_layout(window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    if window.is_minimized()? {
        return Ok(());
    }
    let position = window.outer_position()?;
    let size = window
        .inner_size()?
        .to_logical::<f64>(window.scale_factor()?);
    let layout = WindowLayout {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        pinned: window.is_always_on_top()?,
    };
    let path = layout_path(window)?;
    fs::create_dir_all(path.parent().expect("window layout directory"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec(&layout)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn position_popup(calendar: &WebviewWindow, popup: &WebviewWindow) -> tauri::Result<()> {
    let position = calendar.outer_position()?;
    let calendar_size = calendar.outer_size()?;
    let popup_size = popup.outer_size()?;
    let gap = (10.0 * calendar.scale_factor()?).round() as i32;
    let right = position.x + calendar_size.width as i32 + gap;
    let left = position.x - gap - popup_size.width as i32;
    let prefers_left = popup.label() == MEMO_WINDOW;
    let mut x = if prefers_left { left } else { right };
    let mut y = position.y;
    if let Some(monitor) = calendar.current_monitor()? {
        let area = monitor.work_area();
        let max_x = (area.position.x + area.size.width as i32 - popup_size.width as i32)
            .max(area.position.x);
        let max_y = (area.position.y + area.size.height as i32 - popup_size.height as i32)
            .max(area.position.y);
        if prefers_left && left < area.position.x {
            x = right;
        } else if !prefers_left && right > max_x {
            x = left;
        }
        x = x.clamp(area.position.x, max_x);
        y = y.clamp(area.position.y, max_y);
    }
    popup.set_position(PhysicalPosition::new(x, y))
}

fn open_popup(calendar: &WebviewWindow, label: &str, focus: bool) -> Result<(), String> {
    if calendar.label() != "main" {
        return Err("달력 창에서만 팝업을 열 수 있습니다.".into());
    }
    let (page, title) = if label == MEMO_WINDOW {
        ("memo.html", "Daymark 메모")
    } else {
        ("index.html", "Daymark 체크리스트")
    };
    let result = (|| -> tauri::Result<()> {
        let popup = if let Some(existing) = calendar.get_webview_window(label) {
            existing
        } else {
            let builder = WebviewWindowBuilder::new(calendar, label, WebviewUrl::App(page.into()))
                .title(title)
                .inner_size(340.0, 510.0)
                .decorations(false)
                .resizable(true)
                .maximizable(false)
                .focused(focus)
                .visible(false);
            let builder = if label == MEMO_WINDOW {
                builder.min_inner_size(240.0, 220.0)
            } else {
                builder.min_inner_size(290.0, 290.0).always_on_top(true)
            };
            let popup = builder.build()?;
            let restored = restore_layout(&popup).unwrap_or_else(|error| {
                log::warn!("{label} 창 복원 실패: {error}");
                false
            });
            if !restored {
                position_popup(calendar, &popup)?;
            }
            popup
        };
        popup.show()?;
        remember_popup(calendar, label, true);
        if focus {
            popup.set_focus()?;
        }
        Ok(())
    })();
    result.map_err(|error| error.to_string())
}

#[tauri::command]
async fn open_checklist(calendar: WebviewWindow, toggle: Option<bool>) -> Result<(), String> {
    if toggle.unwrap_or(false) && hide_open_popup(&calendar, CHECKLIST_WINDOW)? {
        return Ok(());
    }
    open_popup(&calendar, CHECKLIST_WINDOW, true)
}

fn hide_open_popup(calendar: &WebviewWindow, label: &str) -> Result<bool, String> {
    if calendar.label() != "main" {
        return Err("달력 창에서만 팝업을 숨길 수 있습니다.".into());
    }
    if let Some(popup) = calendar.get_webview_window(label) {
        if popup.is_visible().map_err(|error| error.to_string())? {
            if let Err(error) = save_layout(&popup) {
                log::warn!("{label} 창 상태 저장 실패: {error}");
            }
            popup.hide().map_err(|error| error.to_string())?;
            remember_popup(calendar, label, false);
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
async fn toggle_memo(calendar: WebviewWindow) -> Result<(), String> {
    if hide_open_popup(&calendar, MEMO_WINDOW)? {
        return Ok(());
    }
    open_popup(&calendar, MEMO_WINDOW, true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(SessionClosing::default())
        .manage(memo_tabs::MemoDropTarget::default())
        .invoke_handler(tauri::generate_handler![
            open_checklist,
            toggle_memo,
            documents::load_document,
            documents::save_document,
            memo_tabs::edit_memo_tab,
            memo_tabs::detach_memo_tab,
            memo_tabs::detached_memo_ids,
            memo_tabs::register_memo_tab_bar,
            memo_tabs::begin_memo_tab_drag,
            memo_tabs::clear_memo_drop_preview,
            memo_tabs::place_memo_tab
        ])
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                )
                && let Some(save) = window.try_state::<CalendarLayoutSave>()
                && let Some(calendar) = window.get_webview_window("main")
            {
                let _ = save.0.send(calendar);
            }
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let closing = window.state::<SessionClosing>();
                if window.label() == "main" {
                    closing.0.store(true, Ordering::Relaxed);
                }
                if let Some(webview) = window.get_webview_window(window.label()) {
                    if POPUPS.contains(&window.label()) && !closing.0.load(Ordering::Relaxed) {
                        remember_popup(&webview, window.label(), false);
                    }
                    if !window.label().starts_with(memo_tabs::DETACHED_PREFIX)
                        && let Err(error) = save_layout(&webview)
                    {
                        log::warn!("{} 창 상태 저장 실패: {error}", window.label());
                    }
                }
            }
            if window.label().starts_with(memo_tabs::DETACHED_PREFIX)
                && matches!(event, tauri::WindowEvent::Destroyed)
            {
                memo_tabs::notify_reattached(window);
            }
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                for label in POPUPS {
                    if let Some(popup) = window.get_webview_window(label) {
                        if let Err(error) = popup.close() {
                            log::warn!("{label} 창 닫기 실패: {error}");
                        }
                    }
                }
                for (label, popup) in window.webview_windows() {
                    if label.starts_with(memo_tabs::DETACHED_PREFIX)
                        && let Err(error) = popup.close()
                    {
                        log::warn!("{label} 창 닫기 실패: {error}");
                    }
                }
            }
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            #[cfg(windows)]
            if let Err(error) = documents::migrate_legacy_data(app) {
                log::warn!("이전 Daymark 데이터 복사 실패: {error}");
            }
            if let Err(error) = reminders::start(app.handle().clone()) {
                log::warn!("일정 알림을 시작하지 못했습니다: {error}");
            }
            if let Some(calendar) = app.get_webview_window("main") {
                if let Err(error) = restore_layout(&calendar) {
                    log::warn!("달력 창 복원 실패: {error}");
                }
                app.manage(CalendarLayoutSave::start()?);
                tauri::async_runtime::spawn(async move {
                    for label in POPUPS {
                        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                            if popup_state_path(&calendar, label)?.try_exists()? {
                                open_popup(&calendar, label, false)?;
                            }
                            Ok(())
                        })();
                        if let Err(error) = result {
                            log::warn!("{label} 팝업 복원 실패: {error}");
                        }
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
