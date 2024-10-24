#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;

use app::steam::SteamModel;
use win_dialog::{WinDialog, style, Icon};

fn main() {
    simple_logging::log_to_file("wait.log", log::LevelFilter::Debug).unwrap();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([400.0, 300.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/wait.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    let start = std::time::Instant::now();

    let mut steam_model = match SteamModel::new() {
        Ok(steam_model) => steam_model,
        Err(err) => {
            WinDialog::new(format!("Error: {}", err))
                .with_style(style::Ok_)
                .with_icon(Icon::Error)
                .show()
                .expect("Failed to show dialog");
            return
        }
    };
    log::info!("Steam model created");
    match steam_model.detect_accounts() {
        Ok(_) => {}
        Err(err) => {
            WinDialog::new(format!("Error: {}", err))
                .with_style(style::Ok_)
                .with_icon(Icon::Error)
                .show()
                .expect("Failed to show dialog");
            return
        }
    }
    log::info!("Accounts detected");
    match steam_model.detect_installs() {
        Ok(_) => {}
        Err(err) => {
            WinDialog::new(format!("Error: {}", err))
                .with_style(style::Ok_)
                .with_icon(Icon::Error)
                .show()
                .expect("Failed to show dialog");
            return
        }
    }
    log::info!("Installs detected");

    match eframe::run_native(
        "wait",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc, steam_model, start)))
        }),
    ) {
        Ok(_) => {}
        Err(err) => {
            WinDialog::new(format!("Error: {}", err))
                .with_style(style::Ok_)
                .with_icon(Icon::Error)
                .show()
                .expect("Failed to show dialog");
        }
    }
}
