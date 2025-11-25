use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct SourceCalendar {
    pub name: String,
    pub description: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CalendarGroup {
    pub name: String,
    pub calendars: Vec<SourceCalendar>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub key: String,
    pub url: String,
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_seconds: u64,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,
    pub calendars: Vec<CalendarGroup>,
    #[serde(skip)]
    pub calendar_map: HashMap<String, Vec<SourceCalendar>>,
}

fn default_server_port() -> u16 {
    5000
}

fn default_cache_ttl() -> u64 {
    300
}

fn default_request_timeout() -> u64 {
    30
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let mut config: Config = serde_json::from_str(&content)
            .context("Failed to parse config.json")?;

        // If WEBCAL_KEY environment variable is set, use it instead of config.json key
        if let Ok(env_key) = std::env::var("WEBCAL_KEY") {
            config.key = env_key;
        }

        // Pre-compute the calendar map
        config.calendar_map = config.calendars
            .iter()
            .map(|group| (group.name.clone(), group.calendars.clone()))
            .collect();

        Ok(config)
    }

    pub fn get_calendar_map(&self) -> &HashMap<String, Vec<SourceCalendar>> {
        &self.calendar_map
    }

    pub fn get_all_calendars(&self) -> Vec<SourceCalendar> {
        self.calendars
            .iter()
            .flat_map(|group| group.calendars.clone())
            .collect()
    }
}
