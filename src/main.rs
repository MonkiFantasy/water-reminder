#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    water_reminder::run_app().await
}
