use argon2::{password_hash::PasswordHashString, Argon2, PasswordVerifier};
use axum::{
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};

pub fn check_auth(
    auth_query: Option<&String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
    hash: &PasswordHashString,
) -> Result<(), (StatusCode, String)> {
    if let Some(auth) = auth_query {
        return check_auth_query(auth, hash);
    }

    if let Some(TypedHeader(auth)) = auth_header {
        return check_auth_header(auth, hash);
    }

    Err((StatusCode::UNAUTHORIZED, "Authorization failed!".to_owned()))
}

/// Checks if user is authorized by checking if the given Bearer Token matches the given hash
pub fn check_auth_header(
    authorization: Authorization<Bearer>,
    hash: &PasswordHashString,
) -> Result<(), (StatusCode, String)> {
    return check_auth_key(authorization.token().as_bytes(), hash);
}

/// Checks if user is authorized by checking if a given query parameter matches the given hash
pub fn check_auth_query(
    authorization: &String,
    hash: &PasswordHashString,
) -> Result<(), (StatusCode, String)> {
    return check_auth_key(authorization.as_bytes(), hash);
}

/// Checks authorization by checking if a (raw) key matches a given hash  
/// Returns 401 (UNAUTHORIZED) with appropriate message if they do not match
pub fn check_auth_key(key: &[u8], hash: &PasswordHashString) -> Result<(), (StatusCode, String)> {
    return match (Argon2::default()).verify_password(key, &hash.password_hash()) {
        Err(err) => {
            // TODO: Might want to distinguish between invalid password (4xx) and internal errors (e.g. cryptographic ones)
            log::error!("Error during authentication: {}", err);
            return Err((StatusCode::UNAUTHORIZED, "Invalid token!".to_string()));
        }
        Ok(_) => Ok(()),
    };
}
