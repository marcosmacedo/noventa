use cfg_if::cfg_if;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "I/O error: {}", err),
            ConfigError::Parse(err) => write!(f, "Parse error: {}", err),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(err: serde_yaml::Error) -> Self {
        ConfigError::Parse(err)
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum SessionBackend {
    Cookie,
    Memory,
    Redis,
}

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug, Default)]
pub struct CoreAllocation {
    pub python_threads: Option<usize>,
    pub template_renderer_threads: Option<usize>,
    pub actix_web_threads: Option<usize>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Config {
    pub server_address: Option<String>,
    pub port: Option<u32>,
    pub core_allocation: Option<CoreAllocation>,
    pub max_memory_size: Option<usize>,
    pub temp_dir: Option<String>,
    pub adaptive_shedding: Option<bool>,
    pub database: Option<String>,
    pub static_path: Option<String>,
    pub static_url_prefix: Option<String>,
    pub session: Option<SessionConfig>,
    pub log_level: Option<String>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

cfg_if! {
    if #[cfg(test)] {
        lazy_static! {
            pub static ref CONFIG: Config = Config::default();
        }
    } else {
        lazy_static! {
            pub static ref CONFIG: Config = match Config::from_file("./config.yaml") {
                Ok(config) => config,
                Err(e) => {
                    match e {
                        ConfigError::Io(_) => {
                            println!("I couldn't find the `config.yaml` file. Make sure it's in the same directory you're running the application from.");
                        },
                        ConfigError::Parse(err) => {
                            println!("There seems to be a syntax error in your `config.yaml` file. Please check the formatting.");
                            println!("Details: {}", err);
                        }
                    }
                    std::process::exit(1);
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_config_from_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.yaml");
        let mut file = File::create(&config_path).unwrap();
        file.write_all(
            b"
server_address: 127.0.0.1
port: 8080
core_allocation:
  python_threads: 4
  template_renderer_threads: 4
  actix_web_threads: 8
max_memory_size: 1024
temp_dir: /tmp
adaptive_shedding: true
database: postgresql://user:pass@localhost/db
static_path: /static
static_url_prefix: /static-prefix
session:
  backend: cookie
  secret_key: a-very-secret-key
  cookie_name: my-session
  cookie_secure: true
  cookie_http_only: true
  cookie_path: /
  cookie_domain: example.com
  cookie_max_age: 3600
",
        )
        .unwrap();

        let config = Config::from_file(config_path.to_str().unwrap()).unwrap();

        assert_eq!(config.server_address, Some("127.0.0.1".to_string()));
        assert_eq!(config.port, Some(8080 as u32));
        let core_allocation = config.core_allocation.unwrap();
        assert_eq!(core_allocation.python_threads, Some(4));
        assert_eq!(core_allocation.template_renderer_threads, Some(4));
        assert_eq!(core_allocation.actix_web_threads, Some(8));
        assert_eq!(config.max_memory_size, Some(1024));
        assert_eq!(config.temp_dir, Some("/tmp".to_string()));
        assert_eq!(config.adaptive_shedding, Some(true));
        assert_eq!(
            config.database,
            Some("postgresql://user:pass@localhost/db".to_string())
        );
        assert_eq!(config.static_path, Some("/static".to_string()));
        assert_eq!(
            config.static_url_prefix,
            Some("/static-prefix".to_string())
        );

        let session = config.session.unwrap();
        assert!(matches!(session.backend, SessionBackend::Cookie));
        assert_eq!(session.secret_key, "a-very-secret-key");
        assert_eq!(session.cookie_name, "my-session");
        assert!(session.cookie_secure);
        assert!(session.cookie_http_only);
        assert_eq!(session.cookie_path, "/");
        assert_eq!(session.cookie_domain, Some("example.com".to_string()));
        assert_eq!(session.cookie_max_age, Some(3600));
    }

    #[test]
    fn test_config_from_invalid_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("invalid_config.yaml");
        let mut file = File::create(&config_path).unwrap();
        file.write_all(b"invalid content").unwrap();

        let result = Config::from_file(config_path.to_str().unwrap());
        assert!(matches!(result, Err(ConfigError::Parse(_))));
    }

    #[test]
    fn test_config_from_file_not_found() {
        let result = Config::from_file("non_existent_config.yaml");
        assert!(matches!(result, Err(ConfigError::Io(_))));
    }
}