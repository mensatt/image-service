use argon2::{password_hash, password_hash::PasswordHashString, Argon2, PasswordVerifier};
use axum::{
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};

/// Checks if user is authorized by checking if the given Bearer Token or query parameter matches
/// the given hashes.
pub fn check_auth(
    auth_query: Option<&String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
    hashes: &Vec<PasswordHashString>,
) -> Result<(), (StatusCode, String)> {
    if let Some(auth) = auth_query {
        return check_auth_query(auth, hashes);
    }

    if let Some(TypedHeader(auth)) = auth_header {
        return check_auth_header(auth, hashes);
    }

    Err((StatusCode::UNAUTHORIZED, "Authorization failed!".to_owned()))
}

/// Checks if user is authorized by checking if the given Bearer Token matches the given hashes.
pub fn check_auth_header(
    authorization: Authorization<Bearer>,
    hashes: &Vec<PasswordHashString>,
) -> Result<(), (StatusCode, String)> {
    return check_auth_key(authorization.token().as_bytes(), hashes);
}

/// Checks if user is authorized by checking if a given query parameter matches the given hash
pub fn check_auth_query(
    authorization: &String,
    hash: &Vec<PasswordHashString>,
) -> Result<(), (StatusCode, String)> {
    return check_auth_key(authorization.as_bytes(), hash);
}

/// Checks authorization by checking if a (raw) key matches a given hash  
/// Returns 401 (UNAUTHORIZED) with appropriate message if they do not match
pub fn check_auth_key(
    key: &[u8],
    hashes: &Vec<PasswordHashString>,
) -> Result<(), (StatusCode, String)> {
    for hash in hashes {
        match (Argon2::default()).verify_password(key, &hash.password_hash()) {
            Ok(_) => return Ok(()),
            Err(err) => {
                match err {
                    password_hash::errors::Error::Password => {
                        // Password is incorrect
                        continue;
                    }
                    _ => {
                        // Some other error occurred
                        log::error!(
                            "Error during authentication: {} for hash={}",
                            err,
                            hash
                        );
                        continue;
                    }
                }
            }
        }
    }
    
    log::warn!("Authentication failed for key: {:?}", key);

    Err((StatusCode::UNAUTHORIZED, "Invalid token!".to_string()))
}
