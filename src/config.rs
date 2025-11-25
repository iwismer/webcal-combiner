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
    pub calendars: Vec<CalendarGroup>,
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

        Ok(config)
    }

    pub fn get_calendar_map(&self) -> HashMap<String, Vec<SourceCalendar>> {
        self.calendars
            .iter()
            .map(|group| (group.name.clone(), group.calendars.clone()))
            .collect()
    }

    pub fn get_all_calendars(&self) -> Vec<(String, Vec<SourceCalendar>)> {
        self.calendars
            .iter()
            .map(|group| (group.name.clone(), group.calendars.clone()))
            .collect()
    }
}
