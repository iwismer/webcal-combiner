use crate::config::SourceCalendar;
use anyhow::{Context, Result};
use icalendar::{Component, Calendar, CalendarComponent};
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Clone)]
pub struct CalendarService {
    client: reqwest::Client,
    cache: Option<Arc<Cache<String, String>>>,
}

impl CalendarService {
    pub fn new(enable_cache: bool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let cache = if enable_cache {
            Some(Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(300)) // 5 minutes default
                    .build(),
            ))
        } else {
            None
        };

        Self { client, cache }
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
            Duration::from_secs(30),
            self.client.get(url).send()
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

        // Create new combined calendar
        let mut combined_cal = Calendar::new();
        combined_cal
            .append_property(icalendar::Property::new("PRODID", name))
            .append_property(icalendar::Property::new("VERSION", "2.0"))
            .append_property(icalendar::Property::new("NAME", name))
            .append_property(icalendar::Property::new("X-WR-CALNAME", name));

        // Process each fetched calendar
        for (source_name, cal_text) in fetched_calendars {
            let parsed = cal_text
                .parse::<Calendar>()
                .map_err(|e| anyhow::anyhow!("Failed to parse calendar '{}': {}", source_name, e))?;

            // Iterate through components
            for component in parsed.components {
                match component {
                    CalendarComponent::Event(mut event) => {
                        // Modify the summary to add the source prefix
                        if let Some(summary_prop) = event.property_value("SUMMARY") {
                            let new_summary = format!("{} [{}]", summary_prop, source_name);
                            event.summary(&new_summary);
                        }
                        combined_cal.push(event);
                    }
                    // Note: The icalendar crate doesn't expose Timezone as a separate enum variant
                    // Timezones are handled internally, so we only need to handle events
                    _ => {
                        // Ignore other component types (alarms, journals, todos, timezones, etc.)
                    }
                }
            }
        }

        Ok(combined_cal.to_string())
    }

    pub async fn combine_all_calendars(
        &self,
        calendar_groups: &[(String, Vec<SourceCalendar>)],
    ) -> Result<String> {
        // Fetch all calendar groups in parallel
        let fetch_tasks: Vec<_> = calendar_groups
            .iter()
            .map(|(name, calendars)| {
                let service = self.clone();
                let name = name.clone();
                let calendars = calendars.clone();
                tokio::spawn(async move {
                    service
                        .generate_combined_calendar(&name, &calendars)
                        .await
                })
            })
            .collect();

        // Wait for all to complete
        let mut combined_calendars = Vec::new();
        for task in fetch_tasks {
            let result = task.await.context("Task panicked")?;
            combined_calendars.push(result?);
        }

        // Join all calendars with newlines
        Ok(combined_calendars.join("\n"))
    }
}
