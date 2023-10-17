use axum::{
    body::Bytes,
    extract::Multipart,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Router,
};
use std::{env, ffi::OsStr, fs, io, net::SocketAddr, path::Path};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Create Router with index and upload endpoints
    let app = Router::new()
        .route("/", get(handler))
        // TODO: Endpoint for image queries
        .route("/upload", post(upload));

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
    return Ok(());
}
