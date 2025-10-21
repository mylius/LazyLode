use std::fs;
use toml;

fn main() {
    let content = fs::read_to_string("config.toml").expect("Failed to read config file");
    match toml::from_str::<toml::Value>(&content) {
        Ok(config) => {
            println!("TOML parsing successful");
            println!("Config keys: {:?}", config.as_table().unwrap().keys().collect::<Vec<_>>());
        }
        Err(e) => {
            println!("TOML parsing error: {}", e);
            std::process::exit(1);
        }
    }
}