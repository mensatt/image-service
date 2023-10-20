mod image_utils;

use axum::{
    body::StreamBody,
    extract::{DefaultBodyLimit, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use image_utils::{determine_file_type, determine_img_dim, manipulate_image, save_unmodified};
use libvips::VipsApp;
use serde::Deserialize;
use std::{ffi::OsStr, path::Path as StdPath};
use tokio_util::io::ReaderStream;
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
        .route("/image/:id", get(image_handler));

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

async fn upload_handler(mut multipart: Multipart) -> Result<String, (StatusCode, String)> {
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

    let extension = match StdPath::new(&name).extension().and_then(OsStr::to_str) {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Filename has no extension!".to_owned(),
            ))
        }
        Some(ext) => ext,
    };

    let file_type = match determine_file_type(&data) {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "File type could not be determined or your file type is not supported!".to_owned(),
            ))
        }
        Some(file_type) => file_type,
    };

    // TODO check if extensions are valid (only certain should be allowed)

    let id = Uuid::new_v4();

    match save_unmodified(data, id, extension, 0.0) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error has occurred!".to_owned(),
            ));
        }
        Ok(_) => (),
    };

    return Ok(id.to_string());
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

    let path = "uploads/".to_owned() + &id.to_string() + ".webp";

    // Check if file exists and send error if it does not
    if !StdPath::new(&path).exists() {
        return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
    }

    let img_dim = match determine_img_dim(&path) {
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

    match manipulate_image(&path, height, width, quality) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while processing image!".to_owned(),
            ));
        }
        Ok(ign) => ign,
    };

    // TODO: Extract to constant
    let path = "temp.webp";
    let filename = "temp.webp";
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err))),
    };
    let content_type = match mime_guess::from_path(&path).first_raw() {
        Some(mime) => mime,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "MIME Type couldn't be determined".to_string(),
            ));
        }
    };

    let stream = ReaderStream::new(file);
    let body = StreamBody::new(stream);

    let headers = [
        (header::CONTENT_TYPE, content_type.to_owned()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{:?}\"", filename),
        ),
    ];
    return Ok((headers, body));
}
