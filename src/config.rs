use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default = "default_api_key")]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    #[serde(default = "default_retry_delay_secs")]
    pub retry_delay_secs: u32,
}

fn default_api_url() -> String {
    "https://api.deepseek.com".into()
}

fn default_api_key() -> String {
    String::new()
}

fn default_model() -> String {
    "deepseek-v4-flash".into()
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

fn default_system_prompt() -> String {
    "You are a helpful coding assistant. Answer concisely and accurately.".into()
}

fn default_retry_count() -> u32 {
    2
}

fn default_retry_delay_secs() -> u32 {
    2
}

impl Config {
    pub fn load() -> Self {
        let file_config = Self::load_from_file();
        Self::apply_env_overrides(file_config)
    }

    fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|mut p| {
            p.push(".zero-code-cli");
            p.push("config.toml");
            p
        })
    }

    fn load_from_file() -> Self {
        let defaults = Self::default();
        match Self::config_path() {
            Some(path) if path.exists() => match fs::read_to_string(&path) {
                Ok(content) => toml::from_str(&content).unwrap_or(defaults),
                Err(_) => defaults,
            },
            _ => defaults,
        }
    }

    fn apply_env_overrides(mut config: Self) -> Self {
        if let Ok(val) = env::var("DEEPSEEK_API_KEY") {
            config.api_key = val;
        }
        if let Ok(val) = env::var("DEEPSEEK_API_URL") {
            config.api_url = val;
        }
        if let Ok(val) = env::var("DEEPSEEK_MODEL") {
            config.model = val;
        }
        config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_url: default_api_url(),
            api_key: default_api_key(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            system_prompt: default_system_prompt(),
            retry_count: default_retry_count(),
            retry_delay_secs: default_retry_delay_secs(),
        }
    }
}
