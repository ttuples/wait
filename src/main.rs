use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::rc::Rc;
use utils::steam;

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
    
    //TODO: Optimize this
    let now = std::time::Instant::now();
    let games: Vec<Game> = detected_games.iter().map(
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
                            default_portrait.clone()
                        }
                    }
                },
                None => default_portrait.clone(),
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
                            default_landscape.clone()
                        }
                    }
                },
                None => default_landscape.clone(),
            };


            Game {
                id: *id,
                name: SharedString::from(name),
                thumbnail: Thumbnail { portrait, landscape },
            }
        }
    ).collect();
    println!("Loading games took: {:?}", now.elapsed());

    // ---------- Setup UI ----------
    let app = AppWindow::new()?;

    app.global::<AppAdapter>().set_accounts(
        ModelRc::from(Rc::new(VecModel::<SharedString>::from(steam_model.user_cache.iter().map(|account| SharedString::from(account.name.clone()))
            .collect::<Vec<SharedString>>())))
    );

    app.global::<AppAdapter>().set_games(
        ModelRc::from(Rc::new(VecModel::<Game>::from(games.clone())))
    );

    //TODO: Load favorites from registry settings
    app.global::<AppAdapter>().set_favorites(
        ModelRc::from(Rc::new(VecModel::<Game>::from(vec![])))
    );

    app.global::<AppAdapter>().on_search_changed({
        let app_handle = app.as_weak();
        let games_handle = games.clone();
        move |search| {
            let app_handle = app_handle.upgrade().unwrap();
            // let games_handle = app_handle.global::<AppAdapter>().get_games();
            let filtered_games = games_handle.iter().filter(|game| game.name.contains(search.as_str())).cloned().collect::<Vec<Game>>();
            app_handle.global::<AppAdapter>().set_games(ModelRc::from(Rc::new(VecModel::<Game>::from(filtered_games))));
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
        move |game| {
            let app_handle = app_handle.upgrade().unwrap();
            println!("Favorite game: {}", game.id);

            let favorites_handle = app_handle.global::<AppAdapter>().get_favorites();
            let favorites_handle = favorites_handle.as_any().downcast_ref::<VecModel<Game>>().unwrap();

            // Loop through and get the index of the game
            if let Some(index) = favorites_handle.iter().position(|g| g.id == game.id) {
                favorites_handle.remove(index);
            } else {
                favorites_handle.push(game.clone());
            }
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
