use slint::ComponentHandle;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tokio::time::{interval, Duration};

// 引入生成的 UI 代码
slint::include_modules!();

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppState {
    current_water: i32,
    last_update: DateTime<Utc>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_water: 0,
            last_update: Utc::now(),
        }
    }
}

// 获取持久化数据路径
fn get_config_path() -> PathBuf {
    // Android 环境下，优先尝试标准的配置目录
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "hacker", "waterreminder") {
        let config_dir = proj_dirs.config_dir();
        let _ = fs::create_dir_all(config_dir);
        return config_dir.join("data.json");
    }
    // 降级方案：当前目录下的 data.json
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
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(state) {
        let _ = fs::write(path, content);
    }
}

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // 在 Android 环境中，Slint 会通过其 android-activity 后端接管生命周期
    
    let ui = MainWindow::new()?;
    let state = load_state();

    ui.set_current_water(state.current_water);

    let quotes = vec![
        "保持水分，保护你的内存安全。",
        "JYY 提醒你：该喝水了。",
        "Rust 程序员从不让自己的身体内存泄漏。",
        "多喝热水，少写 Bug。",
        "你的身体也需要 GC (Great Cup of water)。",
        "喝水是零成本的 Self-care。",
    ];
    let quote_idx = (Utc::now().timestamp() as usize) % quotes.len();
    ui.set_quote(quotes[quote_idx].into());

    let ui_handle = ui.as_weak();

    ui.on_add_water(move |amount| {
        if let Some(ui) = ui_handle.upgrade() {
            let current = ui.get_current_water();
            let goal = ui.get_goal_water();
            let new_current = (current + amount).min(goal);
            ui.set_current_water(new_current);
            
            save_state(&AppState {
                current_water: new_current,
                last_update: Utc::now(),
            });
        }
    });

    let ui_handle_reset = ui.as_weak();
    ui.on_reset_data(move || {
        if let Some(ui) = ui_handle_reset.upgrade() {
            ui.set_current_water(0);
            save_state(&AppState::default());
        }
    });

    // 提醒协程
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let state = load_state();
            if state.current_water < 2000 {
                println!("[NOTIFICATION_PENDING]: 喝水时间到了。");
            }
        }
    });

    ui.run()
}
