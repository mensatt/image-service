use axum::{
    body::{Bytes, StreamBody},
    extract::{Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use libvips::{ops, VipsApp, VipsImage};
use serde::Deserialize;
use std::{env, ffi::OsStr, fs, io, net::SocketAddr, path::Path as StdPath};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

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

    let extension = StdPath::new(&name).extension().and_then(OsStr::to_str);
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

    let image = VipsImage::new_from_buffer(&data, "").unwrap();

    let webp_name = "uploads/".to_owned() + &uuid.to_string() + ".webp";

    let rotated = ops::rotate(&image, 0.0).unwrap();
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
    width: Option<i32>,
    height: Option<i32>,
    quality: Option<u32>,
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
    let path = "temp.jpg";
    let filename = "temp.jpg";
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
            ))
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

fn determine_img_dim(path: &str) -> Result<(i32, i32), libvips::error::Error> {
    let img = match VipsImage::new_from_file(path) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };
    return Ok((img.get_width(), img.get_height()));
}

fn manipulate_image(
    path: &str,
    height: i32,
    width: i32,
    quality: u32,
) -> Result<(), libvips::error::Error> {
    let thumb_opts = ops::ThumbnailOptions {
        height: height,
        // See https://github.com/olxgroup-oss/libvips-rust-bindings/issues/42
        import_profile: "sRGB".into(),
        export_profile: "sRGB".into(),
        crop: ops::Interesting::Centre,
        ..ops::ThumbnailOptions::default()
    };
    let image = match ops::thumbnail_with_opts(path, width, &thumb_opts) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };

    // TODO: Implement quality resizing

    // TODO: Figure out if there is a better way of passing the image.
    // Ideas (tried but failed, worth investigating further): Returning image or writing to buffer
    match image.image_write_to_file("temp.jpg") {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };
    return Ok(());
}
