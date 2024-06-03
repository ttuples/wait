use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use utils::steam::SteamModel;
use std::rc::Rc;
use utils::steam;
use utils::settings;

slint::include_modules!();

mod utils;

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // ---------- Steam ----------
    let mut steam_model = steam::SteamModel::new().unwrap();
    println!("Steam path: {:?}", steam_model.path);

    //TODO: Error handling
    steam_model.detect_accounts().unwrap();

    let detected_games = match steam_model.detect_installs(steam_model.path.clone()) {
        Ok(games) => {
            println!("Detected {} games", games.len());
            games
        },
        Err(e) => {
            println!("Error detecting installs: {:?}", e);
            Vec::new()
        }
    };
    
    let default_portrait = Image::default();
    let default_landscape = Image::default();
    
    let now = std::time::Instant::now();
    let games: Vec<Game> = create_game_data(detected_games, &steam_model, (default_portrait, default_landscape));
    println!("Loading games took: {:?}", now.elapsed());

    let mut settings = settings::WaitSettings::init();
    settings.load();

    // ---------- Setup UI ----------
    let app = AppWindow::new()?;

    app.global::<AppAdapter>().set_accounts(
        ModelRc::from(Rc::new(VecModel::<SharedString>::from(steam_model.user_cache.iter().map(|account| SharedString::from(account.name.clone()))
            .collect::<Vec<SharedString>>())))
    );

    app.global::<AppAdapter>().set_games(
        ModelRc::from(Rc::new(VecModel::<Game>::from(games.clone())))
    );

    app.global::<AppAdapter>().set_favorites(
        ModelRc::from(Rc::new(VecModel::<Game>::from(
            games.iter().filter(|game| settings.favorites.contains(&game.id)).cloned().collect::<Vec<Game>>()
        )))
    );

    app.global::<AppAdapter>().on_search_changed({
        let app_handle = app.as_weak();
        let games_handle = games.clone();
        let settings = settings.clone();
        move |search| {
            let app_handle = app_handle.upgrade().unwrap();
            if !search.is_empty() {
                let filtered_games = games_handle.iter().filter(|game| game.name.to_lowercase().contains(search.to_lowercase().as_str())).cloned().collect::<Vec<Game>>();
                app_handle.global::<AppAdapter>().set_games(ModelRc::from(Rc::new(VecModel::<Game>::from(filtered_games))));
                // Hide favorites
                app_handle.global::<AppAdapter>().set_favorites(ModelRc::from(Rc::new(VecModel::<Game>::from(vec![]))));
            } else {
                app_handle.global::<AppAdapter>().set_games(ModelRc::from(Rc::new(VecModel::<Game>::from(games_handle.clone()))));
                // Show favorites
                app_handle.global::<AppAdapter>().set_favorites(
                    ModelRc::from(Rc::new(VecModel::<Game>::from(
                        games_handle.iter().filter(|game| settings.favorites.contains(&game.id)).cloned().collect::<Vec<Game>>()
                    )))
                );
            }
        }
    });

    app.global::<AppAdapter>().on_game_selected({
        let app_handle = app.as_weak();
        let steam_handle = steam_model.clone();
        move |game| {
            let app_handle = app_handle.upgrade().unwrap();

            app_handle.global::<AppAdapter>().set_selected_game(game.clone());

            let game_accounts = steam_handle.user_cache.iter().filter_map(|account| {
                if account.games.contains(&game.id) {
                    Some(SharedString::from(account.name.clone()))
                } else {
                    None
                }
            }).collect::<Vec<SharedString>>();
            println!("Game accounts: {:?}", game_accounts);

            // Handle setting game accounts for the selected game
            app_handle.global::<AppAdapter>().set_game_accounts(ModelRc::from(Rc::new(VecModel::<SharedString>::from(game_accounts.clone()))));
            app_handle.global::<AppAdapter>().set_selected_account(
                match game_accounts.first() {
                    Some(account) => account.clone(),
                    None => SharedString::from(""),
                }
            );
        }
    });

    app.global::<AppAdapter>().on_game_launch({
        let steam_handle = steam_model.clone();
        move |game, account_name| {
            println!("Launching game: {} with account: {}", game.id, account_name);

            let account = match steam_handle.user_cache.iter().find(|account| account.name == account_name.as_str()) {
                Some(account) => account,
                None => return,
            };
            println!("Account: {:?}", account);

            let result = steam_handle.launch_game(&account, &game.id);
            println!("Launch result: {:?}", result);
        }
    });

    app.global::<AppAdapter>().on_game_favorite({
        let app_handle = app.as_weak();
        let mut settings = settings.clone();
        move |game| {
            let app_handle = app_handle.upgrade().unwrap();
            println!("Favorite game: {}", game.id);

            let favorites_handle = app_handle.global::<AppAdapter>().get_favorites();
            let favorites_handle = favorites_handle.as_any().downcast_ref::<VecModel<Game>>().unwrap();

            // Loop through and get the index of the game
            if let Some(index) = favorites_handle.iter().position(|g| g.id == game.id) {
                favorites_handle.remove(index);
                settings.remove_favorite(game.id);
            } else {
                favorites_handle.push(game.clone());
                settings.add_favorite(game.id);
            }

            settings.save();
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

    app.run()
}

fn create_game_data(games: Vec<(i32, String)>, steam_model: &SteamModel, default_thumbnail: (Image, Image)) -> Vec<Game> {
    games.iter().map(
        |(id, name)| {
            // Fetch thumbnails
            let image_paths = match steam_model.game_thumbnail(&id) {
                Ok(image_data) => image_data,
                Err(e) => {
                    eprintln!("Failed to get thumbnail: {:?}", e);
                    (None, None)
                }
            };

            // Create Image from image paths
            let portrait: slint::Image = match image_paths.0 {
                Some(path) => {
                    match image::open(path) {
                        Ok(data) => {
                            let image_data = data.to_rgba8();
                            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&image_data.as_raw(), image_data.width(), image_data.height());
                            slint::Image::from_rgba8(buffer)
                        },
                        Err(_e) => {
                            // eprintln!("Failed to open image: {:?}", e);
                            default_thumbnail.0.clone()
                        }
                    }
                },
                None => default_thumbnail.0.clone(),
            };

            let landscape: slint::Image = match image_paths.1 {
                Some(path) => {
                    match image::open(path) {
                        Ok(data) => {
                            let image_data = data.to_rgba8();
                            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&image_data.as_raw(), image_data.width(), image_data.height());
                            slint::Image::from_rgba8(buffer)
                        },
                        Err(_e) => {
                            // eprintln!("Failed to open image: {:?}", e);
                            default_thumbnail.1.clone()
                        }
                    }
                },
                None => default_thumbnail.1.clone(),
            };


            Game {
                id: *id,
                name: SharedString::from(name),
                thumbnail: Thumbnail { portrait, landscape },
            }
        }
    ).collect()
}