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
            interval_seconds: 60, // 默认改为 60 秒测试
        }
    }
}

// 获取存储路径
fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "hacker", "waterreminder") {
        let config_dir = proj_dirs.config_dir();
        let _ = fs::create_dir_all(config_dir);
        return config_dir.join("data_v3.json");
    }
    PathBuf::from("data_v3.json")
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

// 通用的日志/通知函数
fn notify_user(title: &str, message: &str) {
    // 这里输出到安卓 logcat，如果是真机可以通过 adb logcat 查看
    println!("[NOTIFICATION] {}: {}", title, message);
    // 注意：真正的安卓弹出通知需要 JNI 调用 Java Context 和 NotificationChannel
    // 这里我们先专注于验证定时器逻辑是否在 60 秒后工作
}

pub async fn run_app() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let state = load_state();

    ui.set_current_water(state.current_water);
    // UI 显示的频率改为秒显示
    ui.set_interval_minutes(state.interval_seconds);

    let quotes = vec![
        "SYSTEM_CHECK: WATER_LEVEL_CRITICAL",
        "PROTOCOL_ENGAGED: AUTO_HYDRATE",
        "MAINTAIN_INTERNAL_COOLING_LIQUID",
    ];
    let quote_idx = (Utc::now().timestamp() as usize) % quotes.len();
    ui.set_quote(quotes[quote_idx].into());

    let (tx, mut rx) = watch::channel(state.interval_seconds);
    let ui_handle = ui.as_weak();

    // 逻辑：增加水量
    let ui_handle_add = ui_handle.clone();
    ui.on_add_water(move |amount| {
        if let Some(ui) = ui_handle_add.upgrade() {
            let current = ui.get_current_water();
            let new_current = (current + amount).min(ui.get_goal_water());
            ui.set_current_water(new_current);
            ui.set_last_event(format!("INTAKE: +{}ML", amount).into());
            
            let mut s = load_state();
            s.current_water = new_current;
            s.last_update = Utc::now();
            save_state(&s);
        }
    });

    // 逻辑：修改频率（秒）
    let tx_clone = tx.clone();
    ui.on_update_interval(move |secs| {
        let mut s = load_state();
        s.interval_seconds = secs;
        save_state(&s);
        let _ = tx_clone.send(secs);
    });

    // 定时器协程 (改为每秒检查并倒计时)
    tokio::spawn(async move {
        let mut secs = *rx.borrow();
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(secs as u64)) => {
                    let s = load_state();
                    if s.current_water < 2000 {
                        notify_user("喝水助手", "检测到水分不足，请立即补充！");
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
