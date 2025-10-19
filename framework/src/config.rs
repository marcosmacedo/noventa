use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct Config {
    pub max_memory_size: Option<usize>,
    pub temp_dir: Option<String>,
    pub adaptive_shedding: Option<bool>,
}

impl Config {
    fn from_file(path: &str) -> Result<Self, serde_yaml::Error> {
        let content = fs::read_to_string(path).expect("Could not read config file");
        serde_yaml::from_str(&content)
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::from_file("config.yaml").unwrap();
}