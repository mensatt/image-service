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
        submit::submit_handler,
        unapprove::unapprove_handler,
        upload::upload_handler,
    },
};

use argon2::password_hash::PasswordHashString;
use axum::{
    extract::DefaultBodyLimit,
    response::Html,
    routing::{delete, get, post},
    Router,
};
use libvips::VipsApp;
use std::{env, thread};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

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

    // Read allowed api key hash from environment variable
    let hash_value = match env::var("API_KEY_HASH") {
        Err(err) => panic!("$API_KEY_HASH is not set ({})", err),
        Ok(val) => val,
    };
    let hash = PasswordHashString::new(&hash_value).unwrap();

    let server_state = ServerState { api_key_hash: hash };

    let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);

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
