use std::str::FromStr;

use axum::http::{HeaderValue, Method};
use config::Config;

/// Parses Axum HTTP Methods from the config property `CORS_ALLOWED_METHODS`
pub fn parse_methods(config: &Config) -> Vec<Method> {
    match config.get::<Vec<String>>("CORS_ALLOWED_METHODS") {
        Err(err) => {
            log::warn!(
                "CORS_ALLOWED_METHODS not specified. Only allowing GET requests. Error was: {}",
                err
            );
            Vec::from([Method::GET])
        }
        Ok(vec) => vec
            .iter()
            .filter_map(|elem| Method::from_str(elem).ok())
            .collect(),
    }
}

/// Parses Axum Header Value from the config property `CORS_ALLOWED_METHODS`
pub fn parse_origins(config: &Config) -> Vec<HeaderValue> {
    match config.get::<Vec<String>>("CORS_ALLOWED_ORIGINS") {
        Err(err) => {
            panic!("CORS_ALLOWED_ORIGINS not specified. Error was: {}", err);
        }
        Ok(vec) => vec
            .iter()
            .filter_map(|elem| HeaderValue::from_str(elem).ok())
            .collect(),
    }
}
