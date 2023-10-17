use axum::{body::Bytes, extract::Multipart, http::StatusCode, response::Html, routing::{get, post}, Router, extract};
use std::{env, ffi::OsStr, fs, io, net::SocketAddr};
use extract::{Path, Query}; // <- this import breaks stuff as Path is also in axum
use libvips::{ops, VipsApp, VipsImage};
use uuid::Uuid;
use serde::Deserialize;

#[tokio::main]
async fn main() {
    env_logger::init();

    let libvips = VipsApp::new("mensatt", true).expect("Could not start libvips");
    libvips.concurrency_set(2);

    // Create Router with index and upload endpoints
    let app = Router::new()
        .route("/", get(handler))
        // TODO: Endpoint for image queries
        .route("/upload", post(upload))
        .route("/image/:id", get(image_handler));

    // Start application on localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    log::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html(
        "<h1>This is the image service of Mensatt.</h1>
        <p>Upload pictures at <a href=\"/uploads\">/uploads</a>.</p>
        <p>Request pictures at <a href=\"/placeholder\">/placeholder</a>.</p>
        ",
    )
}

async fn upload(mut multipart: Multipart) -> Result<String, (StatusCode, String)> {
    let field = multipart.next_field().await;
    if field.is_err() {
        return Err((StatusCode::BAD_REQUEST, "No fields provided!".to_owned()));
    }
    let field = field.unwrap();

    if field.is_none() {
        // TODO: Figure out if/when this can occur
        return Err((StatusCode::BAD_REQUEST, "Placeholder".to_owned()));
    }
    let field = field.unwrap();

    let name = field.name().unwrap().to_string();
    let data = field.bytes().await.unwrap();
    log::info!("Received '{}' with size {}B", name, data.len());

    if data.len() == 0 {
        return Err((StatusCode::BAD_REQUEST, "Empty file provided!".to_owned()));
    }

    // Read the env. variable MAX_UPLOAD_SIZE_MB (integer) and parse it. Defaults to 10mb
    let max_upload_size_mb = env::var("MAX_UPLOAD_SIZE_MB")
        .unwrap_or("10".to_string())
        .parse::<usize>()
        .unwrap_or(10);

    // Check if data is too large
    if data.len() > max_upload_size_mb * 1000 * 1000 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("File too large! Max. size is {}MB", max_upload_size_mb),
        ));
    }

    let extension = Path::new(&name).extension().and_then(OsStr::to_str);
    if extension.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Filename has no extension!".to_owned(),
        ));
    }
    let extension = extension.unwrap();

    // TODO check if extensions are valid (only certain should be allowed)

    let id = Uuid::new_v4();

    let return_value = save_unmodified(data, id, extension);
    if return_value.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "An internal error has occurred!".to_owned(),
        ));
    }

    // TODO: rotate and convert to webp
    return Ok(id.to_string());
}

fn save_unmodified(data: Bytes, uuid: Uuid, extension: &str) -> Result<(), io::Error> {
    // TODO: Path should likely be constructed with some sort of OS util
    let new_name = "uploads/".to_owned() + &uuid.to_string() + "." + extension;

    let return_value = fs::write(&new_name, &data);
    if return_value.is_err() {
        let error = return_value.err().unwrap();
        log::error!("Error while writing '{}': {}", new_name, error);
        return Err(error);
    }

    log::info!("Saved '{}'", new_name);

    // test image conversion
    let image = VipsImage::new_from_buffer(&data, &*"").unwrap();

    let webp_name = "uploads/".to_owned() + &uuid.to_string() + ".webp";

    let rotated = ops::rotate(&image, 90.0).unwrap();
    match ops::webpsave(&rotated, &webp_name) {
        Ok(_) => log::info!("Saved '{}'", webp_name),
        Err(error) => {
            log::error!("Error while writing '{}': {}", webp_name, error);
            // todo: return an error
        }
    }

    // end test image conversion

    return Ok(());
}

#[derive(Deserialize)]
struct ImageQuery {
    width: Option<u32>,
    height: Option<u32>,
    quality: Option<u32>,
}

// This handler should serve images with the given id from the filesystem
// The handler should also accept query parameters for width, height and quality
// Images are resized, and compressed using vips
async fn image_handler(
    Path(id): Path<Uuid>,
    query: Option<Query<ImageQuery>>
) -> Result<Bytes, (StatusCode, String)> {

    // check id
    if id.is_nil() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid id!".to_owned(),
        ));
    }

    let mut quality = 80;
    let mut width = 1920;
    let mut height = 1080;

    query.map(|q| {
        let q = q.0;
        if q.width.is_some() {
            width = q.width.unwrap();
        }
        if q.height.is_some() {
            height = q.height.unwrap();
        }
        if q.quality.is_some() {
            quality = q.quality.unwrap();
        }
    });

    let path = "uploads/".to_owned() + &id.to_string() + ".webp";

    // check if file exists and send error if it does not (json)
    let metadata = fs::metadata(&path);
    if metadata.is_err() {
        return Err((
            StatusCode::NOT_FOUND,
            "Image not found!".to_owned(),
        ));
    }

    let _metadata = metadata.unwrap();
    let _image = VipsImage::new_from_file(&path).unwrap();

    // just return empty bytes
    return Ok(Bytes::new());
}
