mod cleaner;
mod constants;
mod handlers;
mod util;

use crate::{
    cleaner::delete_old_pending_images,
    constants::{CONTENT_LENGTH_LIMIT, LISTEN_ADDR},
    handlers::{
        approve::approve_handler, image::image_handler, submit::submit_handler,
        upload::upload_handler,
    },
};

use argon2::password_hash::PasswordHashString;
use axum::{
    extract::DefaultBodyLimit,
    response::Html,
    routing::{get, post},
    Router,
};
use libvips::VipsApp;
use std::{env, thread};

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

    // Create router with index and upload endpoints
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/upload", post(upload_handler))
        .layer(DefaultBodyLimit::max(CONTENT_LENGTH_LIMIT))
        .route("/submit/:id", post(submit_handler))
        .route("/approve/:id", post(approve_handler))
        .route("/image/:id", get(image_handler))
        .with_state(server_state);

    log::info!("Listening on {}", LISTEN_ADDR);
    axum::Server::bind(&LISTEN_ADDR)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/// A simple handler that prints information about this image service
async fn root_handler() -> Html<&'static str> {
    Html(
        "<h1>This is the image service of Mensatt.</h1>
        <p>The following methods and endpoints are offered:</p>
        <ul>
            <li><code>POST</code> to <code>/upload</code></li>
            <li><code>POST</code> to <code>/submit/:id</code></li>
            <li><code>POST</code> to <code>/approve/:id</code></li>
            <li><code>GET</code> to <code>/image/:id</code></li>
        </ul>
        <p>For more information, take a look at the <a target=\"_blank\" href=\"https://github.com/mensatt/image-service\">GitHub Repository</a></p>
        ",
    )
}
