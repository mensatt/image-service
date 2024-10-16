#![allow(clippy::redundant_field_names)]

mod cleaner;
mod constants;
mod handlers;
mod util;

use crate::{
    cleaner::delete_old_pending_images,
    constants::{CONTENT_LENGTH_LIMIT, LISTEN_ADDR},
    handlers::{
        approve::approve_handler,
        image::{image_delete_handler, image_handler},
        rotate::rotate_handler,
        submit::submit_handler,
        unapprove::unapprove_handler,
        upload::upload_handler,
    },
    util::cors::{parse_methods, parse_origins},
};

use argon2::password_hash::PasswordHashString;

use axum::{
    extract::DefaultBodyLimit,
    response::Html,
    routing::{delete, get, post},
    Router,
};
use config::Config;
use libvips::VipsApp;
use std::{env, thread};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct ServerState {
    pub api_key_hashes: Vec<PasswordHashString>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    // Initialize libvips app
    let libvips = VipsApp::new("mensatt", true).expect("Could not start libvips");
    libvips.concurrency_set(4);

    // Create thread that cleans up old pending files
    thread::spawn(|| {
        delete_old_pending_images();
    });

    // If set, read config from CONFIG_PATH env variable, if not try to read from default path
    let config_path = env::var("CONFIG_PATH").unwrap_or("config.yml".to_string());

    // Get config from config path
    // Options set in environment variables override the properties from config file

    // Use ';' as separator, as argon hashes contain commas
    let env_source = config::Environment::default()
        .list_separator(";")
        .with_list_parse_key("API_KEY_HASHES")
        .with_list_parse_key("CORS_ALLOWED_ORIGINS")
        .with_list_parse_key("CORS_ALLOWED_METHODS")
        .try_parsing(true);

    let config = Config::builder()
        .add_source(config::File::with_name(&config_path).required(false))
        .add_source(env_source)
        .build()
        .expect("Could not build config");

    // Get allowed api key hash from config
    let hash_values: Vec<String> = match config.get("API_KEY_HASHES") {
        Err(err) => panic!("$API_KEY_HASHES is not set ({})", err),
        Ok(val) => val,
    };

    let hashes: Vec<PasswordHashString> = hash_values
        .iter()
        .map(|hv| PasswordHashString::new(hv).expect("Failed to parse hash"))
        .collect();

    log::info!("AUTH: Loaded {:?} password hashes", hashes.len());

    let server_state = ServerState {
        api_key_hashes: hashes,
    };

    // Set up CORS
    let methods = parse_methods(&config);
    let origins = parse_origins(&config);
    log::info!("CORS: Allowing {:?} requests from {:?}.", methods, origins);
    let cors = CorsLayer::new()
        .allow_methods(methods)
        .allow_origin(origins);

    let services = ServiceBuilder::new().layer(cors);

    // Create router with index and upload endpoints
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/upload", post(upload_handler))
        .layer(DefaultBodyLimit::max(CONTENT_LENGTH_LIMIT))
        .route("/submit/:id", post(submit_handler))
        .route("/approve/:id", post(approve_handler))
        .route("/image/:id", get(image_handler))
        .route("/image/:id", delete(image_delete_handler))
        .route("/unapprove/:id", post(unapprove_handler))
        .route("/rotate", post(rotate_handler))
        .layer(services)
        .with_state(server_state);

    log::info!("Listening on {}", LISTEN_ADDR);
    let listener = tokio::net::TcpListener::bind(&LISTEN_ADDR).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

/// A simple handler that prints information about this image service
async fn root_handler() -> Html<&'static str> {
    Html(include_str!("index.html"))
}
