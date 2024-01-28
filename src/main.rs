mod auth_utils;
mod constants;
mod handlers;
mod image_utils;
mod path_utils;

use argon2::password_hash::PasswordHashString;
use auth_utils::check_api_key;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    headers::authorization::{Authorization, Bearer},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Router, TypedHeader,
};

use constants::{CONTENT_LENGTH_LIMIT, LISTEN_ADDR};
use handlers::image::image_handler;
use image_utils::{determine_file_type, determine_img_path, save_pending};
use libvips::VipsApp;
use path_utils::{get_original_path, get_pending_path};
use serde::Deserialize;
use std::{env, fs::rename};
use uuid::Uuid;

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
        .route("/image/:id", get(image_handler))
        .route("/approve/:id", post(approve_handler))
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
        <p>Upload pictures at <a href=\"/uploads\">/uploads</a>.</p>
        <p>Request pictures at <a href=\"/image\">/image/:id</a>.</p>
        <p>Request pictures at <a href=\"/approve\">/approve/:id</a>.</p>
        ",
    )
}

#[derive(Deserialize)]
struct UploadQuery {
    angle: Option<f64>,
}

/// This function handles image uploads. An image is expected to be part of a multipart stream.\
/// Only one image (the first field in the stream) is processed.
///
/// Arguments:
///  - query: HTTP Query parameters
///     - angle: To rotate image before saving. Default 0.
///  - multipart: Multipart stream
async fn upload_handler(
    query: Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<String, (StatusCode, String)> {
    // Get first Multipart field
    let field = match multipart.next_field().await {
        Err(err) => {
            log::error!("{}", err.body_text());
            return Err((StatusCode::BAD_REQUEST, "No fields provided!".to_owned()));
        }
        Ok(next_field) => match next_field {
            None => return Err((StatusCode::BAD_REQUEST, "No fields provided!".to_owned())),
            Some(field) => field,
        },
    };

    // Get name and data from field
    let name = field.name().unwrap().to_string();
    let data = match field.bytes().await {
        Err(err) => {
            log::error!("{}", err.body_text());
            match err.status() {
                StatusCode::PAYLOAD_TOO_LARGE => {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        format!(
                            "Content length limit exceeded!. Max allowed file size is {}B",
                            CONTENT_LENGTH_LIMIT
                        ),
                    ));
                }
                _ => return Err((err.status(), "An error occurred your request".to_owned())),
            }
        }
        Ok(data) => data,
    };
    log::info!("Received '{}' with size {}B", name, data.len());

    if data.len() == 0 {
        return Err((StatusCode::BAD_REQUEST, "Empty file provided!".to_owned()));
    }

    let uuid = Uuid::new_v4();
    let angle = match query.angle {
        Some(x) => x,
        None => 0.0,
    };

    match determine_file_type(&data) {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "File type could not be determined or your file type is not supported!".to_owned(),
            ))
        }
        Some(_) => (),
    };

    match save_pending(&data, uuid, angle) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error has occurred!".to_owned(),
            ));
        }
        Ok(_) => (),
    };

    return Ok(uuid.to_string());
}

// TODO: Add cron pruning
async fn approve_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> Result<String, (StatusCode, String)> {
    check_api_key(authorization, &server_state.api_key_hash)?;

    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    let source_path = match determine_img_path(get_pending_path().to_str().unwrap(), uuid) {
        Err(err) => {
            log::error!("{}", err);
            return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
        }
        Ok(str) => str,
    };

    let target_path = get_original_path().join(source_path.file_name().unwrap().to_str().unwrap());

    match rename(&source_path, &target_path) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while approving image!".to_owned(),
            ));
        }
        Ok(_) => log::info!("Moved '{:?}' to '{:?}'", source_path, target_path),
    };

    Ok(uuid.to_string())
}
