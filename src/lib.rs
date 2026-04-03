use slint::ComponentHandle;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tokio::time::{sleep, Duration};
use tokio::sync::watch;

slint::include_modules!();

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppState {
    current_water: i32,
    last_update: DateTime<Utc>,
    interval_seconds: i32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_water: 0,
            last_update: Utc::now(),
            interval_seconds: 60,
        }
    }
}

fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "hacker", "waterreminder") {
        let config_dir = proj_dirs.config_dir();
        let _ = fs::create_dir_all(config_dir);
        return config_dir.join("data_v4.json");
    }
    PathBuf::from("data_v4.json")
}

fn load_state() -> AppState {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<AppState>(&content) {
                let now = Utc::now();
                if state.last_update.date_naive() == now.date_naive() {
                    return state;
                }
            }
        }
    }
    AppState::default()
}

fn save_state(state: &AppState) {
    let path = get_config_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(content) = serde_json::to_string(state) { let _ = fs::write(path, content); }
}

pub async fn run_app() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let state = load_state();

    ui.set_current_water(state.current_water);
    ui.set_interval_minutes(state.interval_seconds);

    let (tx, mut rx) = watch::channel(state.interval_seconds);
    let ui_handle = ui.as_weak();

    // 逻辑：增加水量
    let ui_handle_add = ui_handle.clone();
    ui.on_add_water(move |amount| {
        if let Some(ui) = ui_handle_add.upgrade() {
            let current = ui.get_current_water();
            let new_current = (current + amount).min(ui.get_goal_water());
            ui.set_current_water(new_current);
            ui.set_last_event(slint::SharedString::from(format!("INTAKE_SUCCESS: +{}ML", amount)));
            ui.set_quote("SYSTEM: STATUS_OPTIMAL".into());
            
            let mut s = load_state();
            s.current_water = new_current;
            s.last_update = Utc::now();
            save_state(&s);
        }
    });

    // 逻辑：修改频率
    let tx_clone = tx.clone();
    ui.on_update_interval(move |secs| {
        let mut s = load_state();
        s.interval_seconds = secs;
        save_state(&s);
        let _ = tx_clone.send(secs);
    });

    // 逻辑：重置
    let ui_handle_reset = ui_handle.clone();
    ui.on_reset_data(move || {
        if let Some(ui) = ui_handle_reset.upgrade() {
            ui.set_current_water(0);
            ui.set_last_event("SYSTEM: CORE_RESET_COMPLETED".into());
            save_state(&AppState::default());
        }
    });

    // 定时器协程：关键修复，现在它能更新 UI 了
    let ui_handle_timer = ui_handle.clone();
    tokio::spawn(async move {
        let mut secs = *rx.borrow();
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(secs as u64)) => {
                    let s = load_state();
                    if s.current_water < 2000 {
                        // 尝试更新 UI 界面上的“控制台日志”
                        if let Some(ui) = ui_handle_timer.upgrade() {
                            let _ = slint::invoke_from_event_loop(move || {
                                ui.set_last_event(slint::SharedString::from("WARNING: CRITICAL_DEHYDRATION_DETECTED"));
                                ui.set_quote(slint::SharedString::from("ACTION_REQUIRED: CONSUME_WATER_IMMEDIATELY"));
                            });
                        }
                    }
                }
                changed = rx.changed() => {
                    if changed.is_ok() {
                        secs = *rx.borrow();
                    }
                }
            }
        }
    });

    ui.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { run_app().await.unwrap(); });
}
