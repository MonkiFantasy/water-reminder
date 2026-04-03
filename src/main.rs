#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // 调用库中的逻辑
    water_reminder::run_app().await
}
