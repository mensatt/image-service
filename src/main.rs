mod image_utils;

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use image_utils::{
    check_cache, determine_file_type, determine_img_dim, determine_img_path, get_cache_entry,
    manipulate_image, save_unmodified,
};
use libvips::VipsApp;
use serde::Deserialize;
use std::{
    fs::{read, rename},
    path::PathBuf,
};
use uuid::Uuid;

const CONTENT_LENGTH_LIMIT: usize = 12 * 1024 * 1024;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Initialize libvips App
    let libvips = VipsApp::new("mensatt", true).expect("Could not start libvips");
    libvips.concurrency_set(2);

    // Create Router with index and upload endpoints
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/upload", post(upload_handler))
        .layer(DefaultBodyLimit::max(CONTENT_LENGTH_LIMIT))
        .route("/image/:id", get(image_handler))
        .route("/approve/:id", post(approve_handler));

    // Start application on localhost:3000
    let addr = "0.0.0.0:3000"
        .parse()
        .expect("Unable to parse socket address");
    log::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root_handler() -> Html<&'static str> {
    Html(
        "<h1>This is the image service of Mensatt.</h1>
        <p>Upload pictures at <a href=\"/uploads\">/uploads</a>.</p>
        <p>Request pictures at <a href=\"/placeholder\">/placeholder</a>.</p>
        ",
    )
}

#[derive(Deserialize)]
struct UploadQuery {
    angle: Option<f64>,
}

async fn upload_handler(
    query: Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<String, (StatusCode, String)> {
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

    let file_identification = match determine_file_type(&data) {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "File type could not be determined or your file type is not supported!".to_owned(),
            ))
        }
        Some(file_ident) => file_ident,
    };

    match save_unmodified(&data, uuid, file_identification, angle) {
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

#[derive(Deserialize)]
struct ImageQuery {
    width: Option<i32>,
    height: Option<i32>,
    quality: Option<i32>,
}

// This handler serves images with the given id from the filesystem
// It accepts optional query parameters for width, height and quality
// Images are resized, and compressed using vips
async fn image_handler(Path(id): Path<Uuid>, query: Query<ImageQuery>) -> impl IntoResponse {
    // Check ID
    if id.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    let base_dir = PathBuf::from("data");

    let path = match determine_img_path(base_dir.join("originals").to_str().unwrap(), id) {
        Err(_) => return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned())),
        Ok(str) => str,
    };

    let img_dim = match determine_img_dim(path.to_str().unwrap()) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while getting image dimensions".to_owned(),
            ));
        }
        Ok(img_dim) => img_dim,
    };

    let img_options = query.0;
    let width = match img_options.width {
        Some(width) => width,
        None => img_dim.0,
    };
    let height = match img_options.height {
        Some(height) => height,
        None => img_dim.1,
    };
    let quality = match img_options.quality {
        Some(quality) => quality,
        None => 100,
    };

    let headers = [
        (header::CONTENT_TYPE, "image/webp".to_owned()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename={:?}.webp", id),
        ),
    ];

    let body = match check_cache(id, height, width, quality) {
        true => read(get_cache_entry(&id.to_string(), height, width, quality)).unwrap(),
        false => match manipulate_image(path.to_str().unwrap(), height, width, quality) {
            Err(err) => {
                log::error!("{}", err);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error while processing image!".to_owned(),
                ));
            }
            Ok(buf) => buf,
        },
    };

    return Ok((headers, body));
}

// TODO: Add auth
// TODO: Add cron pruning
async fn approve_handler(Path(uuid): Path<Uuid>) -> Result<String, (StatusCode, String)> {
    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    let base_path = PathBuf::from("data");

    let source_path = match determine_img_path(base_path.join("uploads").to_str().unwrap(), uuid) {
        Err(_) => return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned())),
        Ok(str) => str,
    };

    let target_path = base_path
        .join("originals")
        .join(source_path.file_name().unwrap().to_str().unwrap());

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
