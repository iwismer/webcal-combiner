use crate::config::SourceCalendar;
use anyhow::{Context, Result};
use moka::future::Cache;
use regex::Regex;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Clone)]
pub struct CalendarService {
    client: reqwest::Client,
    cache: Option<Arc<Cache<String, String>>>,
    config: Arc<crate::config::Config>,
}

impl CalendarService {
    pub fn new(enable_cache: bool, config: Arc<crate::config::Config>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        let cache = if enable_cache {
            Some(Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(config.cache_ttl_seconds))
                    .build(),
            ))
        } else {
            None
        };

        Self { client, cache, config }
    }

    async fn fetch_calendar(&self, url: &str) -> Result<String> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(url).await {
                tracing::debug!("Cache hit for URL: {}", url);
                return Ok(cached);
            }
        }

        tracing::debug!("Fetching calendar from URL: {}", url);

        let response = timeout(
            Duration::from_secs(self.config.request_timeout_seconds),
            self.client.get(url).send(),
        )
        .await
        .context("Request timed out")?
        .context("Failed to send request")?;

        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("HTTP error: {} for URL: {}", status, url);
        }

        let body = response.text().await.context("Failed to read response body")?;

        // Store in cache if enabled
        if let Some(ref cache) = self.cache {
            cache.insert(url.to_string(), body.clone()).await;
        }

        Ok(body)
    }

    pub async fn generate_combined_calendar(
        &self,
        name: &str,
        calendars: &[SourceCalendar],
    ) -> Result<String> {
        // Fetch all calendars in parallel
        let fetch_tasks: Vec<_> = calendars
            .iter()
            .map(|cal| {
                let service = self.clone();
                let url = cal.url.clone();
                let cal_name = cal.name.clone();
                tokio::spawn(async move {
                    service
                        .fetch_calendar(&url)
                        .await
                        .context(format!("Failed to fetch calendar: {}", cal_name))
                })
            })
            .collect();

        // Wait for all fetches to complete
        let mut fetched_calendars = Vec::new();
        for (idx, task) in fetch_tasks.into_iter().enumerate() {
            let result = task.await.context("Task panicked")?;
            fetched_calendars.push((calendars[idx].name.clone(), result?));
        }

        // --- String-based merging ---
        let mut combined_cal_string = String::new();
        combined_cal_string.push_str("BEGIN:VCALENDAR\r\n");
        combined_cal_string.push_str(&format!("PRODID:{}\r\n", name));
        combined_cal_string.push_str("VERSION:2.0\r\n");
        combined_cal_string.push_str(&format!("NAME:{}\r\n", name));
        combined_cal_string.push_str(&format!("X-WR-CALNAME:{}\r\n", name));

        let mut all_timezones = std::collections::HashMap::new();
        let mut all_events = Vec::new();

        let unfold_re = Regex::new(r"\r?\n[ \t]").unwrap();
        let re_tz = Regex::new(r"(?ms)BEGIN:VTIMEZONE.*?END:VTIMEZONE").unwrap();
        let re_event = Regex::new(r"(?ms)BEGIN:VEVENT.*?END:VEVENT").unwrap();
        let re_summary = Regex::new(r"SUMMARY:(.*)").unwrap();
        let re_tzid = Regex::new(r"TZID:(.*)").unwrap();

        for (source_name, cal_text) in &fetched_calendars {
            // Pre-process to "unfold" long lines and normalize all line endings to \n
            let unfolded_cal_text = unfold_re.replace_all(cal_text, "");
            let normalized_cal_text = unfolded_cal_text.replace("\r\n", "\n");

            // Extract timezones
            for cap in re_tz.captures_iter(&normalized_cal_text) {
                let tz_text = cap.get(0).unwrap().as_str();
                if let Some(tzid_match) = re_tzid.captures(tz_text) {
                    let tzid = tzid_match.get(1).unwrap().as_str().trim();
                    all_timezones.entry(tzid.to_string()).or_insert_with(|| tz_text.to_string());
                }
            }

            // Extract and modify events
            for cap in re_event.captures_iter(&normalized_cal_text) {
                let event_text = cap.get(0).unwrap().as_str();
                let new_event_text = if let Some(summary_match) = re_summary.captures(event_text) {
                    let original_summary = summary_match.get(1).unwrap().as_str().trim();
                    let new_summary = format!("SUMMARY:{} [{}]", original_summary, source_name);
                    event_text.replacen(summary_match.get(0).unwrap().as_str(), &new_summary, 1)
                } else {
                    event_text.to_string()
                };
                all_events.push(new_event_text);
            }
        }

        // Append unique timezones, ensuring CRLF endings
        for tz_text in all_timezones.values() {
            combined_cal_string.push_str(&tz_text.trim().replace('\n', "\r\n"));
            combined_cal_string.push_str("\r\n");
        }

        // Append events, ensuring CRLF endings
        for event_text in &all_events {
            combined_cal_string.push_str(&event_text.trim().replace('\n', "\r\n"));
            combined_cal_string.push_str("\r\n");
        }

        combined_cal_string.push_str("END:VCALENDAR\r\n");

        Ok(combined_cal_string)
    }

    pub async fn combine_all_calendars(
        &self,
        calendars: &[SourceCalendar],
    ) -> Result<String> {
        self.generate_combined_calendar("all-calendars", calendars)
            .await
    }
}
