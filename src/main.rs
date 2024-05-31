

slint::include_modules!();

mod utils;

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    

    ui.run()
}
