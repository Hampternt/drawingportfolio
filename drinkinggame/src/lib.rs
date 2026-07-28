//! Drinking game — party drink tracker, mounted under /drinks in the
//! portfolio server or served standalone via the bin target.

pub mod auth;
pub mod db;
pub mod error;
pub mod models;
pub mod render;

/// Everything the crate needs from its host. No portfolio types leak in here.
pub struct Config {
    /// e.g. "sqlite:./drinkinggame.db"
    pub database_url: String,
    /// URL prefix the router is mounted under: "" standalone, "/drinks" nested.
    /// Used only for URL generation in templates/redirects — routing itself
    /// is prefix-agnostic because .nest_service() strips the prefix.
    pub base_path: String,
}
