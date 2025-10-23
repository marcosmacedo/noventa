use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Clone, Copy)]
pub enum SessionBackend {
    Cookie,
    InMemory,
    Redis,
}

#[derive(Deserialize)]
pub struct SessionConfig {
    pub backend: SessionBackend,
    pub secret_key: String,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
    pub cookie_path: String,
    pub cookie_domain: Option<String>,
    pub cookie_max_age: Option<i64>,
    pub redis_url: Option<String>,
    pub redis_pool_size: Option<usize>,
}

#[derive(Deserialize)]
pub struct Config {
    pub max_memory_size: Option<usize>,
    pub temp_dir: Option<String>,
    pub adaptive_shedding: Option<bool>,
    pub database: Option<String>,
    pub static_path: Option<String>,
    pub static_url_prefix: Option<String>,
    pub session: Option<SessionConfig>,
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