#![allow(unused)]

use std::collections::HashMap;

use registry::{Data, Hive, Security};

slint::include_modules!();

static WAIT_SETTINGS: &str = "Software\\WaitApp";
static FAVORITES: &str = "Software\\WaitApp\\Favorites";
static HIDDEN: &str = "Software\\WaitApp\\Hidden";
static ACCOUNTS: &str = "Software\\WaitApp\\Accounts";

#[derive(Debug, Clone, Default)]
pub struct WaitSettings {
    pub favorites: Vec<i32>,
    pub hidden: Vec<i32>,
    pub accounts: HashMap<i32, i64>,
}

impl WaitSettings {
    pub fn new() -> Self {
        Self {
            favorites: Vec::new(),
            hidden: Vec::new(),
            accounts: HashMap::new(),
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
        if Hive::CurrentUser.open(ACCOUNTS, Security::Read).is_err() {
            // Create registry key
            Hive::CurrentUser.create(ACCOUNTS, Security::AllAccess).unwrap();
            println!("Created registry key: {:?}", ACCOUNTS)
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
    
    pub fn add_hidden(&mut self, id: i32) {
        if !self.hidden.contains(&id) {
            self.hidden.push(id);
        }
    }

    pub fn remove_hidden(&mut self, id: i32) {
        if let Some(index) = self.hidden.iter().position(|&x| x == id) {
            self.hidden.remove(index);
        }
    }

    pub fn add_account(&mut self, game: i32, account: i64) {
        self.accounts.insert(game, account);
    }

    pub fn load(&mut self) {
        let mut favorites = vec![];
        let mut hidden = vec![];
        let mut accounts = HashMap::new();

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

        // Load game accounts
        let acc_reg = Hive::CurrentUser.open(ACCOUNTS, Security::Read).unwrap();
        for result in acc_reg.values() {
            match result {
                Ok(value) => {
                    // Convert name to i32
                    match value.name().to_string() {
                        Ok(game) => {
                            match value.data() {
                                Data::U64(account) => {
                                    accounts.insert(game.parse::<i32>().unwrap(), (*account as i64));
                                },
                                _ => {}
                            }
                        },
                        Err(_) => {}
                    }
                },
                Err(_) => {}
            }
        }

        self.favorites = favorites;
        self.hidden = hidden;
        self.accounts = accounts;

        println!("Loaded favorites: {:?}", self.favorites);
        println!("Loaded hidden: {:?}", self.hidden);
        println!("Loaded accounts: {:?}", self.accounts);
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
                        Err(_) => {}
                    }
                },
                Err(_) => {}
            }
        }
        for id in fav_to_save {
            fav_reg.create(id.to_string(), Security::Write).unwrap();
        }

        // Save hidden
        let mut hidden_to_save = self.hidden.to_vec();
        let hidden_reg = Hive::CurrentUser.open(HIDDEN, Security::AllAccess).unwrap();
        for result in hidden_reg.keys() {
            match result {
                Ok(value) => {
                    // Convert name to i32
                    match value.to_string().parse::<i32>() {
                        Ok(id) => {
                            if hidden_to_save.contains(&id) {
                                hidden_to_save.remove(hidden_to_save.iter().position(|&x| x == id).unwrap());
                            } else {
                                hidden_reg.delete(&id.to_string(), false).unwrap();
                            }
                        },
                        Err(_) => {}
                    }
                },
                Err(_) => {}
            }
        }

        // Save accounts
        let acc_reg = Hive::CurrentUser.open(ACCOUNTS, Security::AllAccess).unwrap();
        for (game, account) in self.accounts.iter() {
            acc_reg.set_value(game.to_string(), &Data::U64(*account as u64)).unwrap();
        }
    }
}