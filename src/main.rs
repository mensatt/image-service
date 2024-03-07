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
    // This has to be adapted if support for multiple API keys is needed in the future
    pub api_key_hash: PasswordHashString,
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
    let config = Config::builder()
        .add_source(config::File::with_name(&config_path))
        .add_source(config::Environment::default())
        .build()
        .expect("Could not build config");

    // Get allowed api key hash from config
    let hash_value: String = match config.get("API_KEY_HASH") {
        Err(err) => panic!("$API_KEY_HASH is not set ({})", err),
        Ok(val) => val,
    };
    let hash = PasswordHashString::new(&hash_value).expect("Failed to parse hash");

    let server_state = ServerState { api_key_hash: hash };

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
    axum::Server::bind(&LISTEN_ADDR)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/// A simple handler that prints information about this image service
async fn root_handler() -> Html<&'static str> {
    Html(include_str!("index.html"))
}
