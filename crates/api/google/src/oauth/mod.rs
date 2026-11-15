pub mod authorize;
pub mod token;

pub fn routes(r: &mut httpd::Router) {
    r.get("/oauth/authorize", (), authorize::authorize);
    r.post("/oauth/token", (), token::token);
}
