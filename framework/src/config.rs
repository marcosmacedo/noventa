use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct Config {
    pub max_memory_size: Option<usize>,
    pub temp_dir: Option<String>,
    pub adaptive_shedding: Option<bool>,
    pub database: Option<String>,
}

impl Config {
    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

lazy_static! {
    pub static ref CONFIG: Config = match Config::from_file("./config.yaml") {
        Ok(config) => config,
        Err(_) => {
            println!("Error: config.yaml not found or cannot be read.");
            println!("Please ensure the file exists in the directory and has the correct permissions.");
            std::process::exit(1);
        }
    };
}