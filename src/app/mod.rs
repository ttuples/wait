pub mod steam;
mod custom_popup;
use custom_popup::custom_popup;

use egui::{Align, Color32, ImageSource, Layout, Vec2};
use egui_json_tree::JsonTree;
use egui_notify::Toasts;
use steam::prelude::*;

use core::{f32, fmt};
use std::{collections::HashMap, ops::{Mul, Sub}};
// use std::sync::mpsc::{Sender, Receiver, channel};

macro_rules! steam_launch {
    ($steam_model:expr, $account:expr, $app:expr, $close_after:expr) => {
        match $steam_model.launch_game($account, &$app.id, $close_after) {
            Ok(_) => {},
            Err(e) => {
                log::error!("Launch Error: {}", e);
            }
        }
    };
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    favorites: Vec<AppID>,
    hidden: Vec<AppID>,
    saved_logins: HashMap<AppID, SteamAccount>,
    thumbnail_mode: ThumbnailMode,
    grid_size: f32,
    close_after: CloseAfter,
    theme: Theme,

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
    #[serde(skip)]
    toasts: Toasts,
    // #[serde(skip)]
    // toast_channel: (Sender<String>, Receiver<String>),
    #[serde(skip)]
    theme_popup: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            favorites: Vec::new(),
            hidden: Vec::new(),
            saved_logins: HashMap::new(),
            thumbnail_mode: ThumbnailMode::Portrait,
            grid_size: 200.0,
            close_after: CloseAfter::None,
            theme: Theme::default(),
            theme_popup: false,
            steam_model: SteamModel::default(),
            thumbnail_cache: HashMap::new(),
            selected_account: SteamAccount::default(),
            selected_app: None,
            search_filter: String::default(),
            toasts: Toasts::default(),
            // toast_channel: channel(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
enum ThumbnailMode {
    #[default]
    Portrait,
    Landscape
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Default, Clone, PartialEq, Eq, Hash)]
enum CloseAfter {
    #[default]
    None,
    Launch,
    Login,
    Both
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Clone, PartialEq, Eq, Hash)]
struct Theme {
    pub primary: Color32,
    pub secondary: Color32,
    pub background: Color32,
    pub text: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color32::from_rgb(180, 180, 180),
            secondary: Color32::from_rgb(60, 60, 60),
            background: Color32::from_rgb(27, 27, 27),
            text: Color32::from_rgb(171, 171, 171),
        }
    }
}

impl fmt::Display for CloseAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloseAfter::None => write!(f, "None"),
            CloseAfter::Launch => write!(f, "Launch"),
            CloseAfter::Login => write!(f, "Login"),
            CloseAfter::Both => write!(f, "Both"),
        }
    }
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
        log::info!("Thumbnail cache updated");

        let selected_account = match steam_model.get_current_user() {
            Ok(account) => account,
            Err(_) => {
                match steam_model.user_cache.first() {
                    Some(account) => account.clone(),
                    None => {
                        log::error!("No accounts found");
                        panic!("No accounts found");
                    }
                }
            }
        };
        log::info!("Current loggin account: {}", selected_account.name());

        // Persisted state
        if let Some(storage) = cc.storage {
            let mut app: App = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.steam_model = steam_model;
            app.thumbnail_cache = thumbnail_cache;
            app.selected_account = selected_account;
            log::info!("Restored state");
            return app;
        }

        log::info!("No persisted state found. Applying default state");
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
        self.toasts.show(ctx);

        // Update theme
        let mut visuals = egui::Visuals::dark();

        visuals.override_text_color = Some(self.theme.text);

        visuals.widgets.noninteractive.bg_fill = self.theme.secondary;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.5, self.theme.primary);
        visuals.widgets.inactive.bg_fill = self.theme.secondary;
        visuals.widgets.inactive.weak_bg_fill = self.theme.secondary;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, self.theme.primary);
        visuals.widgets.hovered.weak_bg_fill = self.theme.secondary;
        visuals.widgets.active.bg_fill = self.theme.secondary;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, self.theme.primary);
        visuals.widgets.active.weak_bg_fill = self.theme.secondary;

        ctx.set_visuals(visuals);

        egui::SidePanel::left("side_panel")
            .min_width(200.0)
            .resizable(true)
            .frame(egui::Frame::default().fill(self.theme.background).inner_margin(egui::Margin::same(8.0)))
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
                        self.toasts.info(format!("Logging in as {}", self.selected_account.name()));
                        match self.steam_model.login(
                            &self.selected_account,
                            self.close_after == CloseAfter::Login || self.close_after == CloseAfter::Both
                        ) {
                            Ok(_) => {},
                            Err(e) => {
                                log::error!("Login Error: {}", e);
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
                                    .selected_text(format!("{}", self.saved_logins.get(app).expect("Game account was not properly initialized!").name()))
                                    .show_ui(ui, |ui| {
                                        for steam_account in &self.steam_model.user_cache {
                                            if ui.selectable_value(
                                                &mut *self.saved_logins.get_mut(app).unwrap(),
                                                steam_account.clone(),
                                                steam_account.name(),
                                            ).clicked() {
                                                self.toasts.info(format!("Updated game \"{}\" to account \"{}\"", app.name, steam_account.name()));
                                            }
                                        }
                                    });


                                ui.horizontal(|ui| {
                                    let width = ui.available_width() / 2.0 - ui.spacing().item_spacing.x / 2.0;
                                    if ui.add_sized(Vec2::new(width, 40.0), egui::Button::new("Launch")).clicked() {
                                        self.toasts.info(format!("Launching {}", app.name));
                                        steam_launch!(
                                            self.steam_model,
                                            self.saved_logins.get(app).expect("Game account was not properly initialized!"),
                                            app,
                                            self.close_after == CloseAfter::Launch || self.close_after == CloseAfter::Both
                                        );
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
                                            if let Some(manifest) = self.steam_model.get_app_manifest(&app) {
                                                JsonTree::new("", manifest).show(ui);
                                            }
                                        });
                                    });
                            });
                    }
                });
            }
        );

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(self.theme.background).inner_margin(egui::Margin::same(8.0)))
            .show(ctx, |ui| {
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

                        egui::ComboBox::from_label("Close After")
                            .selected_text(format!("{}", self.close_after))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.close_after, CloseAfter::None, CloseAfter::None.to_string());
                                ui.selectable_value(&mut self.close_after, CloseAfter::Launch, CloseAfter::Launch.to_string());
                                ui.selectable_value(&mut self.close_after, CloseAfter::Login, CloseAfter::Login.to_string());
                                ui.selectable_value(&mut self.close_after, CloseAfter::Both, CloseAfter::Both.to_string());
                            });

                        let response = ui.button("Theme");
                        if response.clicked() {
                            self.theme_popup = !self.theme_popup;
                        };
                        if self.theme_popup {
                            custom_popup(ui, &response, |ui| {
                                ui.color_edit_button_srgba(&mut self.theme.primary);
                                ui.color_edit_button_srgba(&mut self.theme.secondary);
                                ui.color_edit_button_srgba(&mut self.theme.background);
                                ui.color_edit_button_srgba(&mut self.theme.text);

                                if ui.button("Reset").clicked() {
                                    self.theme = Theme::default();
                                }
                            });
                        }
                    });

                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::CollapsingHeader::new(format!("Favorites ({})", self.favorites.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                if self.favorites.len() > 0 {
                                    self.game_grid(ui,
                                        self.favorites.iter().filter_map(|app| {
                                            if !self.hidden.contains(app) {
                                                if self.search_filter.is_empty() {
                                                    Some(app.clone())
                                                } else if app.name.to_lowercase().contains(&self.search_filter) {
                                                    Some(app.clone())
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        }).collect::<Vec<AppID>>()
                                    );
                                }
                            }
                        );
                        
                        ui.separator();

                        egui::CollapsingHeader::new(format!("All Games ({})", self.steam_model.games.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                self.game_grid(ui,
                                    self.steam_model.get_installed_apps().iter().filter_map(|app| {
                                        if !self.hidden.contains(app) {
                                            if self.search_filter.is_empty() {
                                                Some(app.clone())
                                            } else if app.name.to_lowercase().contains(&self.search_filter) {
                                                Some(app.clone())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    }).collect::<Vec<AppID>>()
                                );
                            }
                        );

                        ui.separator();

                        egui::CollapsingHeader::new(format!("Hidden ({})", self.hidden.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                if self.hidden.len() > 0 {
                                    self.game_grid(ui, self.hidden.clone());
                                }
                            }
                        );
                    });
                });
            }
        );
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
        let total_item_width:   f32 = self.grid_size * cols as f32;
        let remaining_space:    f32 = ui.available_width() - total_item_width;
        let spacing:            f32 = if cols > 1 { remaining_space / (cols - 1) as f32 } else { 0.0 };
        let inner_margin:       f32 = 1.5;

        let img_width = self.grid_size - inner_margin.mul(2.0);
        let img_height = match self.thumbnail_mode {
            ThumbnailMode::Portrait => self.grid_size * 1.5,
            ThumbnailMode::Landscape => self.grid_size * 0.75
        }.sub(inner_margin.mul(2.0));

        egui::Grid::new("game_grid")
            .num_columns(cols)
            .spacing(Vec2::new(spacing, spacing))
            .show(ui, |ui| {
                for (i, app) in apps.into_iter().enumerate() {

                    egui::Frame::default()
                        .fill(
                            if self.selected_app == Some(app.clone()) {
                                self.theme.primary
                            } else {
                                self.theme.background
                            }
                        )
                        .rounding(10.0)
                        .inner_margin(inner_margin)
                        .show(ui, |ui| {
                            let response = ui.add(
                                self.get_thumbnail_image(&app)
                                    .fit_to_exact_size(Vec2::new(img_width, img_height))
                                    .rounding(8.5)
                                    .sense(egui::Sense::click())
                            );
        
                            self.game_context(&response, &app);
                            
                            if response.clicked() {
                                self.selected_app = Some(app);
                            }
                        });


                    if (i + 1) % cols == 0 { ui.end_row(); }
                }
            });
    }

    fn game_context(&mut self, response: &egui::Response, app: &AppID) {
        response.context_menu(|ui| {
            if ui.button("Launch").clicked() {
                self.toasts.info(format!("Launching {}", app.name));
                steam_launch!(
                    self.steam_model,
                    self.saved_logins.get(app).expect("Game account was not properly initialized!"),
                    app,
                    self.close_after == CloseAfter::Launch || self.close_after == CloseAfter::Both
                );
                ui.close_menu();
            }
            if self.favorites.contains(&app) {
                if ui.button("Remove from Favorites").clicked() {
                    self.favorites.retain(|x| x != app);
                    self.toasts.info(format!("Removed {} from favorites", app.name));
                    ui.close_menu();
                }
            } else {
                if ui.button("Add to Favorites").clicked() {
                    self.favorites.push(app.clone());
                    self.toasts.info(format!("Added {} to favorites", app.name));
                    ui.close_menu();
                }
            }
            if self.hidden.contains(&app) {
                if ui.button("Unhide").clicked() {
                    self.hidden.retain(|x| x != app);
                    self.toasts.info(format!("Unhidden {}", app.name));
                    ui.close_menu();
                }
            } else {
                if ui.button("Hide").clicked() {
                    self.hidden.push(app.clone());
                    self.toasts.info(format!("Hidden {}", app.name));
                    ui.close_menu();
                }
            }
        });
    }
}