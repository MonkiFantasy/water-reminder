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
        return config_dir.join("data_v5.json");
    }
    PathBuf::from("data.json")
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

    // 加水回调
    let ui_handle_add = ui_handle.clone();
    ui.on_add_water(move |amount: i32| {
        if let Some(ui) = ui_handle_add.upgrade() {
            let current = ui.get_current_water();
            let new_current = (current + amount).min(ui.get_goal_water());
            ui.set_current_water(new_current);
            ui.set_last_event(slint::SharedString::from(format!("INTAKE_ACK: +{}ML", amount)));
            ui.set_quote(slint::SharedString::from("PROTOCOL: HYDRATION_ENGAGED"));
            
            let mut s = load_state();
            s.current_water = new_current;
            s.last_update = Utc::now();
            save_state(&s);
        }
    });

    // 频率回调
    let tx_clone = tx.clone();
    ui.on_update_interval(move |secs: i32| {
        let mut s = load_state();
        s.interval_seconds = secs;
        save_state(&s);
        let _ = tx_clone.send(secs);
    });

    // 重置回调
    let ui_handle_reset = ui_handle.clone();
    ui.on_reset_data(move || {
        if let Some(ui) = ui_handle_reset.upgrade() {
            ui.set_current_water(0);
            ui.set_last_event(slint::SharedString::from("SYSTEM: MEMORY_CLEARED"));
            save_state(&AppState::default());
        }
    });

    // 定时器协程
    let ui_handle_timer = ui_handle.clone();
    tokio::spawn(async move {
        let mut secs = *rx.borrow();
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(secs as u64)) => {
                    let s = load_state();
                    if s.current_water < 2000 {
                        let ui_weak = ui_handle_timer.clone();
                        // 修复：将 Weak 句柄捕获进闭包，在主循环线程内 upgrade
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_last_event(slint::SharedString::from("WARNING: DEHYDRATION_IMMINENT"));
                                ui.set_quote(slint::SharedString::from("ACTION: REPLENISH_LIQUIDS"));
                            }
                        });
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
pub fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        run_app().await.unwrap();
    });
}
