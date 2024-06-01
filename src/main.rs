use std::{path::Path, rc::Rc};
use slint::{spawn_local, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use utils::steam;
use async_channel::unbounded;

slint::include_modules!();

mod utils;

#[derive(Debug, Clone)]
pub struct ThumbnailChannel {
    tx: async_channel::Sender<ThumbnailData>,
    rx: async_channel::Receiver<ThumbnailData>,
}

impl ThumbnailChannel {
    fn new() -> Self {
        let (tx, rx) = unbounded::<ThumbnailData>();
        Self { tx, rx }
    }
}

#[derive(Debug, Clone)]
pub struct ThumbnailData(i32, (Option<SharedPixelBuffer<Rgba8Pixel>>, Option<SharedPixelBuffer<Rgba8Pixel>>));

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // ---------- Steam ----------
    let mut steam_model = steam::SteamModel::new().unwrap();
    println!("Steam path: {:?}", steam_model.path);

    steam_model.detect_accounts().unwrap();

    let _games = match steam_model.detect_installs(steam_model.path.clone()) {
        Ok(games) => {
            println!("Detected {} games", games.len());
            games
        },
        Err(e) => {
            println!("Error detecting installs: {:?}", e);
            Vec::new()
        }
    };

    // let portrait_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(PORTRAIT_BYTES, 300, 450);
    // let landscape_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(LANDSCAPE_BYTES, 460, 215);
    let default_portrait = slint::Image::load_from_path(Path::new("./data/default_portrait.png")).unwrap();
    let default_landscape = slint::Image::load_from_path(Path::new("./data/default_landscape.png")).unwrap();

    let game_vec: Vec<Game> = _games.iter().map(|(id, name)| {
        Game {
            id: *id,
            name: SharedString::from(name),
            thumbnail: Thumbnail { portrait: default_portrait.clone(), landscape: default_landscape.clone() },
        }
    }).collect();
    drop(_games);
    let games_model: Rc<VecModel<Game>> = Rc::new(VecModel::<Game>::from(game_vec));

    // ---------- Setup UI ----------
    let app = AppWindow::new()?;

    app.global::<AppAdapter>().set_accounts(ModelRc::from(Rc::new(VecModel::<SharedString>::from(steam_model.user_cache.iter().map(|account| SharedString::from(account.name.clone())).collect::<Vec<SharedString>>()))));

    app.global::<AppAdapter>().set_games(ModelRc::from(games_model.clone()));

    app.global::<AppAdapter>().on_game_selected({
        let games_handle = games_model.clone();
        let app_handle = app.as_weak();
        let steam_handle = steam_model.clone();
        move |game_id| {
            let app_handle = app_handle.upgrade().unwrap();

            let game = games_handle.iter().find(|game| game.id == game_id).unwrap();

            let game_accounts = steam_handle.user_cache.iter().filter_map(|account| {
                if account.games.contains(&game_id) {
                    Some(SharedString::from(account.name.clone()))
                } else {
                    None
                }
            }).collect::<Vec<SharedString>>();

            println!("Game accounts: {:?}", game_accounts);

            app_handle.global::<AppAdapter>().set_selected_game(game.clone());
            app_handle.global::<AppAdapter>().set_selected_account(SharedString::from(game_accounts.first().unwrap().clone()));
            app_handle.global::<AppAdapter>().set_optional_accounts(ModelRc::from(Rc::new(VecModel::<SharedString>::from(game_accounts))));
        }
    });

    app.global::<AppAdapter>().on_game_launch({
        let steam_handle = steam_model.clone();
        move |game, account_name| {
            println!("Launching game: {} with account: {}", game.id, account_name);

            let account = steam_handle.user_cache.iter().find(|account| account.name == account_name.as_str()).unwrap();
            println!("Account: {:?}", account);

            let result = steam_handle.launch_game(&account, &game.id);
            println!("Launch result: {:?}", result);
        }
    });

    app.global::<AppAdapter>().on_account_login({
        let app_handle = app.as_weak();
        let steam_handle = steam_model.clone();
        move |account_name| {
            let _app_handle = app_handle.upgrade().unwrap();
            println!("Logging into account: {}", account_name);

            let account = steam_handle.user_cache.iter().find(|account| account.name == account_name.as_str()).unwrap();
            println!("Account: {:?}", account);

            let result = steam_handle.login(account);
            println!("Login result: {:?}", result);
        }
    });

    app.global::<AppAdapter>().on_steamdb_open({
        move |game| {
            println!("Opening SteamDB for game: {}", game.id);
            let url = format!("https://steamdb.info/app/{}", game.id);
            open::that(url).unwrap();
        }
    });

    app.global::<AppAdapter>().on_debug({
        // let app_handle = app.as_weak();
        move || {
            // let app_handle = app_handle.upgrade().unwrap();
            println!("Debug clicked");
        }
    });

    // ---------- Thumbnail thread ----------
    let thumbnail_channel = ThumbnailChannel::new();

    // Rx thread
    spawn_local({
        let rx = thumbnail_channel.rx.clone();
        let games_handle = games_model.clone();
        async move {
            loop {
                match rx.recv().await {
                    Ok(image_data) => {
                        for index in 0..games_handle.row_count() {
                            let game = games_handle.row_data(index).unwrap();
                            if game.id == image_data.0 {
                                let mut portrait: Image = default_portrait.clone();
                                let mut landscape: Image = default_landscape.clone();

                                let buffers = image_data.clone().1;
                                
                                if let Some(image) = buffers.0 {
                                    portrait = Image::from_rgba8(image);
                                }

                                if let Some(image) = buffers.1 {
                                    landscape = Image::from_rgba8(image);
                                }

                                let mut new_game = game.clone();
                                new_game.thumbnail = Thumbnail {
                                    portrait: portrait.clone(),
                                    landscape: landscape.clone(),
                                };

                                games_handle.set_row_data(index, new_game.clone());
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to receive thumbnail: {:?}", e);
                        continue;
                    }
                }
            }
        }
    }).unwrap();

    // Tx thread
    spawn_local({
        let tx = thumbnail_channel.tx.clone();
        let games_handle = games_model.clone();
        async move {
            // Get gameid of all detected games
            let games_vec = games_handle.iter().map(|game| game.id).collect::<Vec<i32>>();

            tokio::spawn(async move {
                for appid in games_vec {
                    let image_paths = match steam_model.game_thumbnail(&appid) {
                        Ok(image_data) => image_data,
                        Err(e) => {
                            eprintln!("Failed to get thumbnail: {:?}", e);
                            continue;
                        }
                    };

                    // Create SharedPixelBuffer from image paths
                    let portrait: Option<SharedPixelBuffer<Rgba8Pixel>> = match image_paths.0 {
                        Some(path) => {
                            match image::open(path) {
                                Ok(data) => {
                                    let image_data = data.to_rgba8();
                                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&image_data.as_raw(), image_data.width(), image_data.height());
                                    Some(buffer)
                                },
                                Err(_e) => {
                                    // eprintln!("Failed to open image: {:?}", e);
                                    None
                                }
                            }
                        },
                        None => None,
                    };

                    let landscape: Option<SharedPixelBuffer<Rgba8Pixel>> = match image_paths.1 {
                        Some(path) => {
                            match image::open(path) {
                                Ok(data) => {
                                    let image_data = data.to_rgba8();
                                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&image_data.as_raw(), image_data.width(), image_data.height());
                                    Some(buffer)
                                },
                                Err(_e) => {
                                    // eprintln!("Failed to open image: {:?}", e);
                                    None
                                }
                            }
                        },
                        None => None,
                    };

                    tx.send_blocking(ThumbnailData {
                        0: appid,
                        1: (portrait, landscape),
                    }).unwrap();
                }
            }).await.unwrap();
        }
    }).unwrap();

    app.run()
}
