use slint::ComponentHandle;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tokio::time::{interval, Duration};
use tokio::sync::watch;

slint::include_modules!();

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppState {
    current_water: i32,
    last_update: DateTime<Utc>,
    interval_minutes: i32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_water: 0,
            last_update: Utc::now(),
            interval_minutes: 60,
        }
    }
}

// 获取存储路径
fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "hacker", "waterreminder") {
        let config_dir = proj_dirs.config_dir();
        let _ = fs::create_dir_all(config_dir);
        return config_dir.join("data_v2.json");
    }
    PathBuf::from("data_v2.json")
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

// 安卓原生通知函数（通过 JNI 调用）
// 注意：在非安卓环境下此函数为空
#[cfg(target_os = "android")]
fn send_android_notification(title: &str, message: &str) {
    // 这是一个简化的 JNI 调用示例，实际中可能需要处理 Context 和 Channel
    // 对于 Slint/Android-activity 环境，由于没有直接暴露 Context，通常通过特定方式获取
    // 这里我们先在后台输出日志，模拟触发
    println!("[ANDROID_NOTIFY] {}: {}", title, message);
}

#[cfg(not(target_os = "android"))]
fn send_android_notification(title: &str, message: &str) {
    println!("[DESKTOP_NOTIFY] {}: {}", title, message);
}

pub async fn run_app() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let state = load_state();

    // 同步初始状态到 UI
    ui.set_current_water(state.current_water);
    ui.set_interval_minutes(state.interval_minutes);

    let quotes = vec![
        "保持水分，保护你的内存安全。",
        "JYY 提醒你：该喝水了。",
        "Rust 程序员从不让自己的身体内存泄漏。",
        "多喝热水，少写 Bug。",
        "喝水是零成本的 Self-care。",
    ];
    let quote_idx = (Utc::now().timestamp() as usize) % quotes.len();
    ui.set_quote(quotes[quote_idx].into());

    let (tx, mut rx) = watch::channel(state.interval_minutes);
    let ui_handle = ui.as_weak();

    // 逻辑：增加水量
    let ui_handle_add = ui_handle.clone();
    ui.on_add_water(move |amount| {
        if let Some(ui) = ui_handle_add.upgrade() {
            let current = ui.get_current_water();
            let goal = ui.get_goal_water();
            let new_current = (current + amount).min(goal);
            ui.set_current_water(new_current);
            ui.set_last_event(format!("EVENT: ADDED {}ML", amount).into());
            
            let mut s = load_state();
            s.current_water = new_current;
            s.last_update = Utc::now();
            save_state(&s);
        }
    });

    // 逻辑：修改频率
    let tx_clone = tx.clone();
    ui.on_update_interval(move |mins| {
        let mut s = load_state();
        s.interval_minutes = mins;
        save_state(&s);
        let _ = tx_clone.send(mins);
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

    // 异步定时提醒逻辑
    tokio::spawn(async move {
        let mut mins = *rx.borrow();
        loop {
            // 根据当前频率设置定时器
            let trigger_duration = Duration::from_secs(mins as u64 * 60);
            
            tokio::select! {
                _ = tokio::time::sleep(trigger_duration) => {
                    let s = load_state();
                    if s.current_water < 2000 {
                        send_android_notification("喝水助手", "检测到水分摄入不足，请立即补充！");
                    }
                }
                changed = rx.changed() => {
                    if changed.is_ok() {
                        mins = *rx.borrow();
                        println!("SYSTEM: 提醒频率已更新为 {} 分钟", mins);
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
