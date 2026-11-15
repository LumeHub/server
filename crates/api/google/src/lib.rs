pub mod commands;
pub mod device;
pub mod effects;
pub mod endpoint;
pub mod error;
pub mod oauth;
pub mod request;
pub mod response;

pub use endpoint::GoogleState;

/// Smart Home fulfillment plus the (stub) OAuth account-linking endpoints.
pub fn routes(r: &mut httpd::Router, st: &GoogleState) {
    endpoint::routes(r, st);
    oauth::routes(r);
}
