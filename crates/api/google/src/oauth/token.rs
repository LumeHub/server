use httpd::{BadRequest, Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenRequest {
    pub grant_type: String,
    #[allow(dead_code)]
    pub client_id: String,
    pub refresh_token: Option<String>,
    #[allow(dead_code)]
    pub code: Option<String>,
    #[allow(dead_code)]
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i32,
}

pub async fn token(_: (), req: Request) -> Result<Response, BadRequest> {
    let form: TokenRequest = req.form()?;
    Ok(match form.grant_type.as_str() {
        "authorization_code" => Response::json(
            200,
            &TokenResponse {
                access_token: "generated-access-token".to_string(),
                refresh_token: "generated-refresh-token".to_string(),
                token_type: "Bearer".to_string(),
                expires_in: 3600,
            },
        ),
        "refresh_token" => {
            if let Some(refresh_token) = &form.refresh_token {
                Response::json(
                    200,
                    &TokenResponse {
                        access_token: "refreshed-access-token".to_string(),
                        refresh_token: refresh_token.clone(),
                        token_type: "Bearer".to_string(),
                        expires_in: 3600,
                    },
                )
            } else {
                Response::text(400, "Missing refresh_token")
            }
        }
        _ => Response::text(400, "Unsupported grant_type"),
    })
}
