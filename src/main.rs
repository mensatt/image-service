mod image_utils;

use axum::{
    body::{Bytes, StreamBody},
    extract::{DefaultBodyLimit, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use libvips::{ops, VipsApp, VipsImage};
use serde::Deserialize;
use std::{ffi::OsStr, fs, io, path::Path as StdPath};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const CONTENT_LENGTH_LIMIT: usize = 12 * 1024 * 1024;

// Custom error handler that is called when the content length limit is exceeded
async fn content_length_limit_exceeded_handler() -> (StatusCode, &'static str) {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        "Content-Length limit exceeded!",
    )
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let libvips = VipsApp::new("mensatt", true).expect("Could not start libvips");
    libvips.concurrency_set(2);

    // Create Router with index and upload endpoints
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/upload", post(upload_handler))
        //.layer(DefaultBodyLimit::disable())
        .layer(DefaultBodyLimit::max(CONTENT_LENGTH_LIMIT))
        // .map_err(content_length_limit_exceeded_handler)
        .route("/image/:id", get(image_handler));

    // Start application on localhost:3000
    let addr = "0.0.0.0:3000".parse().expect("Unable to parse socket address");
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

    // // Read the env. variable MAX_UPLOAD_SIZE_MB (integer) and parse it. Defaults to 10mb
    // let max_upload_size_mb = env::var("MAX_UPLOAD_SIZE_MB")
    //     .unwrap_or("10".to_string())
    //     .parse::<usize>()
    //     .unwrap_or(10);
    //
    // // Check if data is too large
    // if data.len() > max_upload_size_mb * 1000 * 1000 {
    //     return Err((
    //         StatusCode::BAD_REQUEST,
    //         format!("File too large! Max. size is {}MB", max_upload_size_mb),
    //     ));
    // }

    let extension = StdPath::new(&name).extension().and_then(OsStr::to_str);
    if extension.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Filename has no extension!".to_owned(),
        ));
    }
    let extension = extension.unwrap();

    let file_type = image_utils::determine_file_type(&data);
    if file_type.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "File type could not be determined or your file type is not supported!".to_owned(),
        ));
    }

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

    let opts = ops::WebpsaveOptions {
        // TODO: Do not convert images to webp on upload, save them as is
        // WEBP-lossless only inflates image (tested 1.3 MB JPEG => ~3 MB WEBP)
        // lossless: true,
        ..ops::WebpsaveOptions::default()
    };
    match ops::webpsave_with_opts(&rotated, &webp_name, &opts) {
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
    quality: i32,
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

    // TODO: Figure out if there is a better way of passing the image.
    // Ideas (tried but failed, worth investigating further): Returning image or writing to buffer
    let opts = ops::WebpsaveOptions {
        q: quality,
        ..ops::WebpsaveOptions::default()
    };
    match ops::webpsave_with_opts(&image, "temp.webp", &opts) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };
    return Ok(());
}
