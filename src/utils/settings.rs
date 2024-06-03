#![allow(unused)]

use registry::{Hive, Security};

slint::include_modules!();

static WAIT_SETTINGS: &str = "Software\\WaitApp";
static FAVORITES: &str = "Software\\WaitApp\\Favorites";
static HIDDEN: &str = "Software\\WaitApp\\Hidden";

#[derive(Debug, Clone)]
pub struct WaitSettings {
    pub favorites: Vec<i32>,
    pub hidden: Vec<i32>,
}

impl WaitSettings {
    pub fn new() -> Self {
        Self {
            favorites: vec![],
            hidden: vec![],
        }
    }

    pub fn init() -> Self {
        // Check if registry keys exists
        if Hive::CurrentUser.open(WAIT_SETTINGS, Security::Read).is_err() {
            // Create registry key
            Hive::CurrentUser.create(WAIT_SETTINGS, Security::AllAccess).unwrap();
            println!("Created registry key: {:?}", WAIT_SETTINGS)
        }
        if Hive::CurrentUser.open(FAVORITES, Security::Read).is_err() {
            // Create registry key
            Hive::CurrentUser.create(FAVORITES, Security::AllAccess).unwrap();
            println!("Created registry key: {:?}", FAVORITES)
        }
        if Hive::CurrentUser.open(HIDDEN, Security::Read).is_err() {
            // Create registry key
            Hive::CurrentUser.create(HIDDEN, Security::AllAccess).unwrap();
            println!("Created registry key: {:?}", HIDDEN)
        }

        Self::new()
    }

    pub fn add_favorite(&mut self, id: i32) {
        if !self.favorites.contains(&id) {
            self.favorites.push(id);
        }
    }

    pub fn remove_favorite(&mut self, id: i32) {
        if let Some(index) = self.favorites.iter().position(|&x| x == id) {
            self.favorites.remove(index);
        }
    }
    
    pub fn load(&mut self) {
        let mut favorites = vec![];
        let mut hidden = vec![];

        // Load favorites
        let fav_reg = Hive::CurrentUser.open(FAVORITES, Security::Read).unwrap();
        for result in fav_reg.keys() {
            match result {
                Ok(value) => {
                    // Convert name to i32
                    match value.to_string().parse::<i32>() {
                        Ok(id) => {
                            favorites.push(id);
                        },
                        Err(e) => {
                            println!("Error: {:?}", e);
                        }
                    }
                },
                Err(e) => println!("Error: {:?}", e)
            }
        }

        // Load hidden
        let hidden_reg = Hive::CurrentUser.open(HIDDEN, Security::Read).unwrap();
        for result in hidden_reg.keys() {
            match result {
                Ok(value) => {
                    // Convert name to i32
                    match value.to_string().parse::<i32>() {
                        Ok(id) => {
                            hidden.push(id);
                        },
                        Err(_) => {
                        }
                    }
                },
                Err(_) => {}
            }
        }

        println!("Loaded favorites: {:?}", favorites);
        println!("Loaded hidden: {:?}", hidden);

        self.favorites = favorites;
        self.hidden = hidden;
    }

    pub fn save(&self) {
        // Save favorites
        let mut fav_to_save = self.favorites.to_vec();
        let fav_reg = Hive::CurrentUser.open(FAVORITES, Security::AllAccess).unwrap();
        for result in fav_reg.keys() {
            match result {
                Ok(value) => {
                    // Convert name to i32
                    match value.to_string().parse::<i32>() {
                        Ok(id) => {
                            if fav_to_save.contains(&id) {
                                fav_to_save.remove(fav_to_save.iter().position(|&x| x == id).unwrap());
                            } else {
                                fav_reg.delete(&id.to_string(), false).unwrap();
                            }
                        },
                        Err(_) => {
                        }
                    }
                },
                Err(_) => {}
            }
        }
        for id in fav_to_save {
            fav_reg.create(id.to_string(), Security::Write).unwrap();
        }

        // Save hidden
    }
}