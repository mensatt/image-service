use crate::util::auth::check_auth_header;
use crate::util::cache::{cache_status, CacheInformation};
use crate::ServerState;
use axum::extract::{Path, State};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::StatusCode;
use axum::TypedHeader;
use uuid::Uuid;

pub async fn cache_status_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> Result<String, (StatusCode, String)> {
    // All cache operations need to be secured
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    match cache_status() {
        Ok(info) => Ok(info.to_string()),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not collect cache information!".to_string(),
        )),
    }
}

pub async fn precache_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> Result<String, (StatusCode, String)> {
    // All cache operations need to be secured
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    Ok("".to_string())
}

pub async fn purge_cache_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> Result<String, (StatusCode, String)> {
    // All cache operations need to be secured
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    Ok("".to_string())
}

pub async fn delete_cache_entry_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> Result<String, (StatusCode, String)> {
    // All cache operations need to be secured
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    Ok("".to_string())
}
