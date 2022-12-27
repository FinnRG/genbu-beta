//! # Genbu Authentication
//!
//! This crate contains utility functions for authentication.
//!
//! ## Password hashing
//!
//! ```
//! use genbu_auth::authn::*;
//! use secrecy::SecretString;
//!
//! let password = SecretString::new(String::from("Test"));
//! let wrong_password = SecretString::new(String::from("Test2"));
//!
//! let hash = hash_password(&password).unwrap();
//! assert!(verify_password(&password, &hash).unwrap());
//! assert!(!verify_password(&wrong_password, &hash).unwrap());
//! ```
//!
//! ## JSON-WebToken
//!
//! ```
//! use genbu_auth::authn::*;
//! use genbu_stores::Uuid;
//!
//! let jwt = create_jwt(Uuid::new_v4());
//! assert!(jwt.is_ok());
//! ```

use std::ops::Add;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use jsonwebtoken::errors::{Error as ExtJWTError, ErrorKind as ExtJWTErrorKind};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use password_hash::SaltString;
use rand_core::OsRng;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{ext::NumericalDuration, OffsetDateTime};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HashError {
    #[error("hash function error")]
    Hash(#[from] password_hash::Error),
}

fn normalize(pass: &SecretString) -> SecretString {
    SecretString::new(pass.expose_secret().nfkc().collect::<String>())
}

/// Creates a hash with the given password.
///
/// # Errors
///
/// This function will return an error only if the crpto library errrors internally, which should
/// never happen for a valid string.
pub fn hash_password(password: &SecretString) -> Result<String, HashError> {
    let password = normalize(password);
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.expose_secret().as_bytes(), &salt)?;
    let s = hash.serialize();
    Ok(s.as_str().to_owned())
}

/// Verifies that the given password results in the given hash.
///
/// # Errors
///
/// This function will return an error only if the crypto library errors internally, which should
/// never happen for a valid string and a valid hash.
#[tracing::instrument(name = "Validate password", skip_all)]
pub fn verify_password(password: &SecretString, hash: &str) -> Result<bool, HashError> {
    let pass = normalize(password);
    let argon2 = Argon2::default();
    let result = argon2.verify_password(pass.expose_secret().as_bytes(), &PasswordHash::new(hash)?);
    match result {
        Ok(_) => Ok(true),
        Err(password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    exp: i64,
}

impl Claims {
    #[must_use]
    pub fn new(id: Uuid) -> Self {
        let exp = OffsetDateTime::now_utc().add(6.hours()).unix_timestamp();
        Self {
            sub: id.to_string(),
            exp,
        }
    }
}

/// All of the possible errors which can occur during JWT creation and validation. If it isn't
/// clear whether the JWT was tampered with or if there was an internal crypto error,
/// [`JWTErrorKind::Invalid`] is usually preferred.
#[non_exhaustive]
#[derive(Debug)]
pub enum JWTErrorKind {
    /// The JWT library wasn't correctly set up
    Configuration,
    /// Something unspecified went wrong with crypto/serializing
    Internal,
    /// When the JWT isn't valid
    Invalid,
}

#[derive(Debug, Error)]
#[error("jwt error")]
pub struct JWTError {
    kind: JWTErrorKind,
    #[source]
    source: jsonwebtoken::errors::Error,
}

/// Creates a JWT for the given id.
///
/// # Errors
///
/// This function will return an error only if the internal crypto libary errors, which can only
/// happen if the supplied secret is invalid.
#[tracing::instrument(name = "Create new JSON-WebToken", skip_all)]
pub fn create_jwt(id: Uuid) -> Result<String, JWTError> {
    let claims = Claims::new(id);
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(
            b"secret", //TODO: Make this
                      //configurable
        ),
    )
    .map_err(Into::into)
}

/// Decodes a JWT and returns the claims.
///
/// # Errors
///
/// This function will return an error if the decoding key if invalid, the crypto library errors
/// internally or the JWT was tampered with.
#[tracing::instrument(name = "Validate JSON-WebToken", skip_all)]
pub fn validate_jwt(jwt: &str) -> Result<Claims, JWTError> {
    match jsonwebtoken::decode::<Claims>(
        jwt,
        &DecodingKey::from_secret(b"secret"), // TODO: Make this configurable
        &Validation::default(),
    ) {
        Ok(data) => Ok(data.claims),
        Err(e) => Err(e.into()),
    }
}

#[cfg(feature = "http")]
use http::StatusCode;

#[cfg(feature = "http")]
impl From<HashError> for StatusCode {
    fn from(_: HashError) -> Self {
        // It's safe to assume, that a hash error is always an internal error
        Self::INTERNAL_SERVER_ERROR
    }
}

impl From<&ExtJWTErrorKind> for JWTErrorKind {
    fn from(value: &ExtJWTErrorKind) -> Self {
        match value {
            ExtJWTErrorKind::InvalidEcdsaKey | ExtJWTErrorKind::InvalidRsaKey(_) => {
                Self::Configuration
            }
            ExtJWTErrorKind::Crypto(_) => Self::Internal,
            _ => Self::Invalid, // This generally prefers Invalid over Internal
        }
    }
}

impl From<ExtJWTError> for JWTError {
    fn from(value: ExtJWTError) -> Self {
        Self {
            kind: JWTErrorKind::from(value.kind()),
            source: value,
        }
    }
}

#[cfg(feature = "http")]
impl From<JWTError> for StatusCode {
    fn from(value: JWTError) -> Self {
        match value.kind {
            JWTErrorKind::Invalid => Self::UNAUTHORIZED,
            JWTErrorKind::Internal | JWTErrorKind::Configuration => Self::INTERNAL_SERVER_ERROR,
        }
    }
}
