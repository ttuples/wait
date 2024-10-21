pub mod steam;
use egui::{Align, ImageSource, Layout, Vec2};
use egui_json_tree::JsonTree;
use steam::prelude::*;

use core::f32;
use std::collections::HashMap;

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Default)]
#[serde(default)]
pub struct App {
    favorites: Vec<AppID>,
    saved_logins: HashMap<AppID, SteamAccount>,
    thumbnail_mode: ThumbnailMode,
    grid_size: f32,

    #[serde(skip)]
    steam_model: SteamModel,
    #[serde(skip)]
    thumbnail_cache: HashMap<AppID, Thumbnail>,
    #[serde(skip)]
    selected_account: SteamAccount,
    #[serde(skip)]
    selected_app: Option<AppID>,
    #[serde(skip)]
    search_filter: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
enum ThumbnailMode {
    #[default]
    Portrait,
    Landscape
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, steam_model: SteamModel) -> Self {
        // Update thumbnail cache
        let thumbnail_cache: HashMap<AppID, Thumbnail> = steam_model
            .get_installed_apps()
            .iter()
            .map(|app| {
                let thumbnail: Thumbnail = match steam_model.game_thumbnail(&app.id) {
                    Ok(thumbnail) => thumbnail,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        Thumbnail::default()
                    }
                };
                (app.clone(), thumbnail)
            })
            .collect();

        let selected_account = steam_model.get_current_user().unwrap();

        // Persisted state
        if let Some(storage) = cc.storage {
            let mut app: App = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.grid_size = 200.0;
            app.steam_model = steam_model;
            app.thumbnail_cache = thumbnail_cache;
            app.selected_account = selected_account;
            return app;
        }

        // Default state
        Self {
            grid_size: 200.0,
            steam_model,
            thumbnail_cache,
            selected_account,
            ..Default::default()
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("side_panel")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.heading("Steam Account");

                    egui::ComboBox::from_id_salt("Accounts")
                        .width(ui.available_width())
                        .selected_text(format!("{}", self.selected_account.name()))
                        .show_ui(ui, |ui| {
                            for steam_account in &self.steam_model.user_cache {
                                ui.selectable_value(
                                    &mut self.selected_account,
                                    steam_account.clone(),
                                    steam_account.name(),
                                );
                            }
                        });
                    
                    if ui.button("Login to Steam").clicked() {
                        match self.steam_model.login(&self.selected_account) {
                            Ok(_) => {
                                //TODO: Option to close app after logging in
                            },
                            Err(e) => {
                                eprintln!("Error: {}", e);
                            }
                        }
                    }

                    ui.separator();

                    if let Some(app) = &self.selected_app {
                        ui.heading(app.name.to_string());

                        let thumbnail: Thumbnail = self.thumbnail_cache.get(&app).unwrap_or(&Thumbnail::default()).clone();
                        if let Some(portrait) = thumbnail.portrait {
                            ui.add(
                                egui::Image::new(format!("file://{}", portrait.to_string_lossy()))
                                    .maintain_aspect_ratio(true)
                                    .max_height(250.0)
                            );
                        } else {
                            ui.add(
                                egui::Image::new(ImageSource::Uri(format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/library_600x900.jpg", app.id).into()))
                                    .maintain_aspect_ratio(true)
                                    .max_height(250.0)
                            );
                        }

                        ui.separator();
                        
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 60.0),
                            Layout::top_down_justified(Align::Center),
                            |ui| {
                                if self.saved_logins.get(app).is_none() {
                                    self.saved_logins.insert(app.clone(), self.selected_account.clone());
                                }

                                egui::ComboBox::from_id_salt("Game Account")
                                    .width(ui.available_width())
                                    .selected_text(format!("{}", self.saved_logins.get(app).unwrap_or(&self.selected_account).name()))
                                    .show_ui(ui, |ui| {
                                        for steam_account in &self.steam_model.user_cache {
                                            if ui.selectable_value(
                                                &mut self.saved_logins.get_mut(app).unwrap(),
                                                &mut steam_account.clone(),
                                                steam_account.name(),
                                            ).clicked() {
                                                self.saved_logins.insert(app.clone(), steam_account.clone());
                                            };
                                        }
                                    });


                                ui.horizontal(|ui| {
                                    let width = ui.available_width() / 2.0 - ui.spacing().item_spacing.x / 2.0;
                                    if ui.add_sized(Vec2::new(width, 40.0), egui::Button::new("Launch")).clicked() {
                                        match self.steam_model.launch_game(self.saved_logins.get(app).unwrap_or(&self.selected_account), &app.id) {
                                            Ok(_) => {},
                                            Err(e) => {
                                                eprintln!("Error: {}", e);
                                            }
                                        }
                                    }
                                    if ui.add_sized(Vec2::new(width, 40.0), egui::Button::new("SteamDB")).clicked() {
                                        let url = format!("https://steamdb.info/app/{}", app.id);
                                        open::that(url).unwrap();
                                    }
                                });
                            },
                        );

                        egui::Frame::default()
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_width(ui.available_width())
                                    .show(ui, |ui| {
                                        ui.expand_to_include_x(ui.available_width());
                                        ui.collapsing("Manifest", |ui| {
                                            if let Some(manifest) = self.steam_model.get_manifest_json(&app) {
                                                JsonTree::new("", manifest).show(ui);
                                            }
                                        });
                                    });
                            });
                    }
                });
            }
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("Steam Library");
                    ui.radio_value(&mut self.thumbnail_mode, ThumbnailMode::Portrait, "Portrait");
                    ui.radio_value(&mut self.thumbnail_mode, ThumbnailMode::Landscape, "Landscape");
                    ui.add(
                        egui::Slider::new(&mut self.grid_size, 30.0..=400.0)
                            .text("Grid Size")
                            .step_by(10.0)
                            .clamping(egui::SliderClamping::Always)
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_filter)
                            .hint_text("Search")
                            .desired_width(300.0)
                    );
                });

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.collapsing("Favorites", |ui| {
                        if self.favorites.len() > 0 {
                            self.game_grid(ui,
                                match self.search_filter {
                                    ref s if s.is_empty() => self.favorites.clone(),
                                    ref s => self.favorites.clone().iter().filter_map(|app| {
                                        if app.name.to_lowercase().contains(s) {
                                            Some(app.clone())
                                        } else {
                                            None
                                        }
                                    }).collect()
                                }
                            );
                        }
                    });
                    ui.separator();
                    ui.collapsing("All Games", |ui| {
                        self.game_grid(ui,
                            match self.search_filter {
                                ref s if s.is_empty() => self.steam_model.get_installed_apps(),
                                ref s => self.steam_model.get_installed_apps().iter().filter_map(|app| {
                                    if app.name.to_lowercase().contains(s) {
                                        Some(app.clone())
                                    } else {
                                        None
                                    }
                                }).collect()
                            }
                        );
                    });
                });
            });
        });
    }
}

impl App {
    fn get_thumbnail_image(&self, app: &AppID) -> egui::Image {
        let thumbnail: Thumbnail = self.thumbnail_cache.get(&app).unwrap_or(&Thumbnail::default()).clone();

        match self.thumbnail_mode {
            ThumbnailMode::Portrait => {
                if let Some(portrait) = thumbnail.portrait {
                    return egui::Image::new(format!("file://{}", portrait.to_string_lossy()));
                } else {
                    return egui::Image::new(ImageSource::Uri(format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/library_600x900.jpg", app.id).into()));
                }
            },
            ThumbnailMode::Landscape => {
                if let Some(landscape) = thumbnail.landscape {
                    return egui::Image::new(format!("file://{}", landscape.to_string_lossy()));
                } else {
                    return egui::Image::new(ImageSource::Uri(format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/header.jpg", app.id).into()));
                }
            }
        }
    }

    fn game_grid<T>(&mut self, ui: &mut egui::Ui, apps: T)
        where T: IntoIterator<Item = AppID>
    {
        let cols = (ui.available_width() / self.grid_size).floor() as usize;
        // let rows = apps.len().div_ceil(cols);
        let total_item_width = self.grid_size * cols as f32;
        let remaining_space = ui.available_width() - total_item_width;
        let spacing = if cols > 1 { remaining_space / (cols - 1) as f32 } else { 0.0 };

        let img_width = self.grid_size;
        let img_height = match self.thumbnail_mode {
            ThumbnailMode::Portrait => self.grid_size * 1.5,
            ThumbnailMode::Landscape => self.grid_size * 0.75
        };

        egui::Grid::new("game_grid")
            .num_columns(cols)
            .spacing(Vec2::new(spacing, spacing))
            .show(ui, |ui| {
                for (i, app) in apps.into_iter().enumerate() {
                    let response = ui.add(
                        self.get_thumbnail_image(&app)
                            .fit_to_exact_size(Vec2::new(img_width, img_height))
                            .rounding(10.0)
                            .sense(egui::Sense::click())
                    );

                    self.game_context(&response, &app);

                    if response.clicked() {
                        self.selected_app = Some(app);
                    }

                    if (i + 1) % cols == 0 { ui.end_row(); }
                }
            });
    }

    fn game_context(&mut self, response: &egui::Response, app: &AppID) {
        response.context_menu(|ui| {
            if ui.button("Launch").clicked() {
                self.launch(app);
                ui.close_menu();
            }
            if self.favorites.contains(&app) {
                if ui.button("Remove from Favorites").clicked() {
                    self.favorites.retain(|x| x != app);
                    ui.close_menu();
                }
            } else {
                if ui.button("Add to Favorites").clicked() {
                    self.favorites.push(app.clone());
                    ui.close_menu();
                }
                if ui.button("Hide").double_clicked() { // Only show hide option for non-favorites
                    ui.close_menu();
                }
            }
        });
    }

    fn launch(&mut self, app: &AppID) {
        match self.steam_model.launch_game(self.saved_logins.get(app).unwrap_or(&self.selected_account), &app.id) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}