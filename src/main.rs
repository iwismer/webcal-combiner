mod calendar;
mod config;

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use calendar::CalendarService;
use config::Config;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    calendar_service: CalendarService,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "webcal_combiner=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = match Config::load("config.json") {
        Ok(config) => {
            tracing::info!("Configuration loaded successfully");
            Arc::new(config)
        }
        Err(e) => {
            tracing::error!("Failed to load config.json: {:?}", e);
            std::process::exit(1);
        }
    };

    // Check if caching is enabled
    let enable_cache = std::env::var("ENABLE_CACHE")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if enable_cache {
        tracing::info!("Calendar caching is ENABLED (5 minute TTL)");
    } else {
        tracing::info!("Calendar caching is DISABLED");
    }

    // Create calendar service
    let calendar_service = CalendarService::new(enable_cache, Arc::clone(&config));

    // Create shared state
    let state = AppState {
        config,
        calendar_service,
    };

    // Get server port
    let server_port = state.config.server_port;

    // Build router
    let app = Router::new()
        .route("/", get(health_check))
        .route("/listing", get(listing))
        .route("/calendar/{key}/{cal_name}", get(get_calendar))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::SERVER,
            header::HeaderValue::from_static("webcal-combiner-rust"),
        ))
        .with_state(state);

    // Start server
    let addr = format!("0.0.0.0:{}", server_port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind to port {}: {:?}", server_port, e);
            std::process::exit(1);
        }
    };

    tracing::info!("Server listening on {}", addr);

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server failed to start: {:?}", e);
        std::process::exit(1);
    }
}

async fn health_check() -> impl IntoResponse {
    ""
}

async fn listing(State(state): State<AppState>) -> impl IntoResponse {
    let mut output = String::new();

    for group in &state.config.calendars {
        output.push_str(&format!(
            "{}: {}/calendar/{{key}}/{}\n",
            group.name, state.config.url, group.name
        ));

        for cal in &group.calendars {
            output.push_str(&format!(
                "  - {} ({}): {}\n",
                cal.name, cal.description, cal.url
            ));
        }

        output.push('\n');
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        output,
    )
}

async fn get_calendar(
    State(state): State<AppState>,
    Path((key, cal_name)): Path<(String, String)>,
) -> Response {
    // Constant-time comparison for key validation
    let key_valid: bool = key.as_bytes().ct_eq(state.config.key.as_bytes()).into();

    if !key_valid {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // Handle "all-calendars" special case
    if cal_name == "all-calendars" {
        let all_calendars = state.config.get_all_calendars();

        match state
            .calendar_service
            .combine_all_calendars(&all_calendars)
            .await
        {
            Ok(calendar_data) => {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    "text/calendar; charset=utf-8".parse().unwrap(),
                );
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=all-calendars.ics".parse().unwrap(),
                );

                (StatusCode::OK, headers, calendar_data).into_response()
            }
            Err(e) => {
                tracing::error!("Failed to generate all-calendars: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to generate calendar: {}", e),
                )
                    .into_response()
            }
        }
    } else {
        // Get specific calendar
        let calendar_map = state.config.get_calendar_map();

        match calendar_map.get(&cal_name) {
            Some(calendars) => {
                match state
                    .calendar_service
                    .generate_combined_calendar(&cal_name, calendars)
                    .await
                {
                    Ok(calendar_data) => {
                        let mut headers = HeaderMap::new();
                        headers.insert(
                            header::CONTENT_TYPE,
                            "text/calendar; charset=utf-8".parse().unwrap(),
                        );
                        headers.insert(
                            header::CONTENT_DISPOSITION,
                            "attachment; filename=calendar.ics".parse().unwrap(),
                        );

                        (StatusCode::OK, headers, calendar_data).into_response()
                    }
                    Err(e) => {
                        tracing::error!("Failed to generate calendar '{}': {:?}", cal_name, e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to generate calendar: {}", e),
                        )
                            .into_response()
                    }
                }
            }
            None => (
                StatusCode::NOT_FOUND,
                format!("Calendar '{}' not found", cal_name),
            )
                .into_response(),
        }
    }
}
