//! The JWT_SECRET rule every Mimir binary starts through
//! (`mimir_core_ai::config::resolve_jwt_secret`). Pure function: no env access.
//!
//! Lives here rather than in a `#[cfg(test)]` module because the crate's lib
//! test target does not compile on main (unrelated test modules).

use mimir_core_ai::config::{
    ALLOW_INSECURE_DEV_JWT_ENV, INSECURE_DEV_JWT_SECRET, JwtSecretError, PUBLIC_JWT_SECRETS,
    resolve_jwt_secret,
};

#[test]
fn a_private_secret_is_used_as_is() {
    assert_eq!(
        resolve_jwt_secret(Some("4f9c0d3e-private-random"), false),
        Ok("4f9c0d3e-private-random".to_string())
    );
}

#[test]
fn missing_or_blank_is_refused() {
    for blank in [None, Some(""), Some("  ")] {
        assert_eq!(
            resolve_jwt_secret(blank, false),
            Err(JwtSecretError::Missing),
            "{blank:?} must be refused"
        );
    }
}

#[test]
fn the_old_default_and_repo_placeholders_are_refused() {
    for public in PUBLIC_JWT_SECRETS {
        assert_eq!(
            resolve_jwt_secret(Some(public), false),
            Err(JwtSecretError::PublicValue),
            "{public} must be refused"
        );
    }
    assert_eq!(
        resolve_jwt_secret(Some(" dev_secret_key\n"), false),
        Err(JwtSecretError::PublicValue)
    );
}

#[test]
fn the_dev_opt_in_allows_the_default() {
    assert_eq!(
        resolve_jwt_secret(None, true),
        Ok(INSECURE_DEV_JWT_SECRET.to_string())
    );
    assert_eq!(
        resolve_jwt_secret(Some("dev_secret_key"), true),
        Ok("dev_secret_key".to_string())
    );
    assert_eq!(
        resolve_jwt_secret(Some("4f9c0d3e-private-random"), true),
        Ok("4f9c0d3e-private-random".to_string())
    );
}

#[test]
fn the_error_names_the_fix() {
    let msg = JwtSecretError::Missing.to_string();
    assert!(msg.contains("JWT_SECRET"), "{msg}");
    assert!(msg.contains(ALLOW_INSECURE_DEV_JWT_ENV), "{msg}");
    assert!(msg.contains("Refusing to start"), "{msg}");
}
