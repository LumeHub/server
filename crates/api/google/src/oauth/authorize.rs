use httpd::{BadRequest, Request, Response};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    redirect_uri: String,
    state: String,
}

pub async fn authorize(_: (), req: Request) -> Result<Response, BadRequest> {
    let query: AuthorizeQuery = req.query()?;
    let redirect_uri = format!(
        "{}?code=fake-auth-code&state={}",
        query.redirect_uri, query.state
    );
    Ok(Response::redirect(&redirect_uri))
}
