use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs, io,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

#[derive(Deserialize)]
struct Schedule {
    id: String,
    text: String,
    #[serde(default)]
    end: i64,
    #[serde(default)]
    lead: i64,
    #[serde(default)]
    done: bool,
}

impl Schedule {
    fn reminder_key(&self) -> String {
        format!("{}:{}:{}", self.id, self.end, self.lead)
    }

    fn due(&self, now: i64) -> bool {
        !self.done
            && matches!(self.lead, 60 | 120)
            && now < self.end
            && now >= self.end.saturating_sub(self.lead * 60_000)
    }
}

fn read_json<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
) -> Result<T, Box<dyn std::error::Error>> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error.into()),
    }
}

fn check_reminders(app: &AppHandle, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let schedules: Vec<Schedule> = read_json(&dir.join("tasks.json"))?;
    let path = dir.join("schedule-reminders.json");
    let mut sent: BTreeSet<String> = read_json(&path)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
    let mut changed = false;
    let active: BTreeSet<_> = schedules
        .iter()
        .filter(|schedule| schedule.end > now)
        .map(Schedule::reminder_key)
        .collect();
    let previous_len = sent.len();
    sent.retain(|key| active.contains(key));
    changed |= sent.len() != previous_len;

    for schedule in schedules.iter().filter(|schedule| schedule.due(now)) {
        let key = schedule.reminder_key();
        if !sent.contains(&key) {
            let minutes = (schedule.end - now + 59_999) / 60_000;
            match app
                .notification()
                .builder()
                .title("Daymark 일정 알림")
                .body(format!("{} · 종료까지 약 {minutes}분", schedule.text))
                .show()
            {
                Ok(()) => {
                    sent.insert(key);
                    changed = true;
                }
                Err(error) => log::warn!("일정 알림을 보내지 못했습니다: {error}"),
            }
        }
    }
    if changed {
        crate::documents::write_data(&path, &serde_json::to_string(&sent)?)?;
    }
    Ok(())
}

pub fn start(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_local_data_dir()?;
    std::thread::Builder::new()
        .name("daymark-reminders".into())
        .spawn(move || {
            loop {
                if let Err(error) = check_reminders(&app, &dir) {
                    log::warn!("일정 알림 확인 실패: {error}");
                }
                std::thread::sleep(Duration::from_secs(10));
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_is_due_only_in_selected_window_before_end() {
        for lead in [60, 120] {
            let mut event = Schedule {
                id: "event".into(),
                text: "회의".into(),
                end: 20_000_000,
                lead,
                done: false,
            };
            let threshold = event.end - lead * 60_000;
            assert!(!event.due(threshold - 1));
            assert!(event.due(threshold));
            assert!(event.due(event.end - 1));
            assert!(!event.due(event.end));
            event.done = true;
            assert!(!event.due(threshold));
            event.done = false;
            event.lead = 0;
            assert!(!event.due(threshold));
        }
    }
}
