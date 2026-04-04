use slint::ComponentHandle;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tokio::time::{sleep, Duration};
use tokio::sync::watch;
use once_cell::sync::OnceCell;

slint::include_modules!();

// 全局存储安卓应用上下文，供后台通知使用
#[cfg(target_os = "android")]
static ANDROID_APP: OnceCell<slint::android::AndroidApp> = OnceCell::new();

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
                if state.last_update.date_naive() == now.date_naive() { return state; }
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

// --- 安卓原生通知实现 ---
#[cfg(target_os = "android")]
fn trigger_system_notification(title: &str, body: &str) {
    let app = match ANDROID_APP.get() {
        Some(a) => a,
        None => return,
    };

    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM).unwrap() };
    let mut env = vm.attach_current_thread().unwrap();
    let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };

    // 获取 NotificationManager
    let context_class = env.find_class("android/content/Context").unwrap();
    let nm_field = env.get_static_field(context_class, "NOTIFICATION_SERVICE", "Ljava/lang/String;").unwrap().l().unwrap();
    let notification_manager = env.call_method(&activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[(&nm_field).into()]).unwrap().l().unwrap();

    // 创建通知渠道 (Android 8.0+)
    let channel_id = env.new_string("water_reminder_channel").unwrap();
    let channel_name = env.new_string("Water Reminder Alerts").unwrap();
    let importance = 4; // IMPORTANCE_HIGH
    let channel_class = env.find_class("android/app/NotificationChannel").unwrap();
    let channel_obj = env.new_object(channel_class, "(Ljava/lang/String;Ljava/lang/CharSequence;I)V", &[(&channel_id).into(), (&channel_name).into(), importance.into()]).unwrap();
    
    env.call_method(&notification_manager, "createNotificationChannel", "(Landroid/app/NotificationChannel;)V", &[(&channel_obj).into()]).unwrap();

    // 构建通知
    let builder_class = env.find_class("android/app/Notification$Builder").unwrap();
    let builder = env.new_object(builder_class, "(Landroid/content/Context;Ljava/lang/String;)V", &[(&activity).into(), (&channel_id).into()]).unwrap();

    let title_str = env.new_string(title).unwrap();
    let body_str = env.new_string(body).unwrap();
    
    // 设置标题、内容、小图标（这里使用系统默认的图标 ID 17301543 即 stat_sys_warning）
    env.call_method(&builder, "setContentTitle", "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;", &[(&title_str).into()]).unwrap();
    env.call_method(&builder, "setContentText", "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;", &[(&body_str).into()]).unwrap();
    env.call_method(&builder, "setSmallIcon", "(I)Landroid/app/Notification$Builder;", &[17301543.into()]).unwrap();
    env.call_method(&builder, "setAutoCancel", "(Z)Landroid/app/Notification$Builder;", &[true.into()]).unwrap();

    let notification = env.call_method(builder, "build", "()Landroid/app/Notification;", &[]).unwrap().l().unwrap();
    
    // 发送通知 (ID = 1)
    env.call_method(notification_manager, "notify", "(ILandroid/app/Notification;)V", &[1.into(), (&notification).into()]).unwrap();
}

#[cfg(not(target_os = "android"))]
fn trigger_system_notification(title: &str, body: &str) {
    println!("[DESKTOP_NOTIFY] {}: {}", title, body);
}

pub async fn run_app() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let state = load_state();

    ui.set_current_water(state.current_water);
    ui.set_interval_minutes(state.interval_seconds);

    let (tx, mut rx) = watch::channel(state.interval_seconds);
    let ui_handle = ui.as_weak();

    // 回调逻辑
    let ui_handle_add = ui_handle.clone();
    ui.on_add_water(move |amount: i32| {
        if let Some(ui) = ui_handle_add.upgrade() {
            let current = ui.get_current_water();
            let new_current = (current + amount).min(ui.get_goal_water());
            ui.set_current_water(new_current);
            ui.set_last_event(slint::SharedString::from(format!("INTAKE_ACK: +{}ML", amount)));
            let mut s = load_state();
            s.current_water = new_current;
            s.last_update = Utc::now();
            save_state(&s);
        }
    });

    let tx_clone = tx.clone();
    ui.on_update_interval(move |secs: i32| {
        let mut s = load_state();
        s.interval_seconds = secs;
        save_state(&s);
        let _ = tx_clone.send(secs);
    });

    let ui_handle_timer = ui_handle.clone();
    tokio::spawn(async move {
        let mut secs = *rx.borrow();
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(secs as u64)) => {
                    let s = load_state();
                    if s.current_water < 2000 {
                        // 1. 发送安卓系统级通知 (弹出横幅)
                        trigger_system_notification("喝水提醒", "检测到水分摄入不足，请立即补充！");
                        
                        // 2. 同时更新 App 内的控制台
                        let ui_weak = ui_handle_timer.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_last_event(slint::SharedString::from("WARNING: SYSTEM_NOTIFICATION_SENT"));
                                ui.set_quote(slint::SharedString::from("ACTION: CHECK_ANDROID_STATUS_BAR"));
                            }
                        });
                    }
                }
                changed = rx.changed() => { if changed.is_ok() { secs = *rx.borrow(); } }
            }
        }
    });

    ui.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::AndroidApp) {
    let _ = ANDROID_APP.set(app.clone());
    slint::android::init(app).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { run_app().await.unwrap(); });
}
