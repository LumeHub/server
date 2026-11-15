use domain::ParamDef;
use effects::BuiltinEffect;
use httpd::{Request, Response, Router};
use serde::{Deserialize, Serialize};
use store::EffectRecord;

use crate::ApiState;
use crate::error::{ApiError, ApiResult};

#[derive(Serialize)]
struct EffectResponse {
    id: String,
    name: String,
    script: String,
    params: Vec<ParamDef>,
    builtin: bool,
}

impl From<EffectRecord> for EffectResponse {
    fn from(r: EffectRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            script: r.script,
            params: r.params,
            builtin: false,
        }
    }
}

impl From<&BuiltinEffect> for EffectResponse {
    fn from(b: &BuiltinEffect) -> Self {
        Self {
            id: b.id(),
            name: b.name.clone(),
            script: b.script.clone(),
            params: b.params.clone(),
            builtin: true,
        }
    }
}

pub async fn list_effects(st: ApiState, _req: Request) -> ApiResult {
    let user_effects = st.store.get_effects().await?;
    let mut effects: Vec<EffectResponse> = st.builtins.iter().map(EffectResponse::from).collect();
    effects.extend(user_effects.into_iter().map(EffectResponse::from));
    Ok(Response::json(200, &effects))
}

#[derive(Deserialize)]
struct EffectBody {
    name: String,
    script: String,
    #[serde(default)]
    params: Vec<ParamDef>,
}

pub async fn create_effect(st: ApiState, req: Request) -> ApiResult {
    let body: EffectBody = req.json()?;
    let record = st
        .store
        .create_effect(&body.name, &body.script, &body.params)
        .await?;
    Ok(Response::json(201, &EffectResponse::from(record)))
}

pub async fn get_effect(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    if let Some(b) = find_builtin(&st.builtins, &id) {
        return Ok(Response::json(200, &EffectResponse::from(b)));
    }
    let record = st.store.get_effect(&id).await?;
    Ok(Response::json(200, &EffectResponse::from(record)))
}

pub async fn update_effect(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    let body: EffectBody = req.json()?;
    if find_builtin(&st.builtins, &id).is_some() {
        return Err(ApiError::Forbidden("cannot modify a built-in effect"));
    }
    let record = st
        .store
        .update_effect(&id, &body.name, &body.script, &body.params)
        .await?;
    Ok(Response::json(200, &EffectResponse::from(record)))
}

pub async fn delete_effect(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    if find_builtin(&st.builtins, &id).is_some() {
        return Err(ApiError::Forbidden("cannot delete a built-in effect"));
    }
    st.store.delete_effect(&id).await?;
    Ok(Response::empty(204))
}

fn find_builtin<'a>(builtins: &'a [BuiltinEffect], id: &str) -> Option<&'a BuiltinEffect> {
    builtins.iter().find(|b| b.id() == id)
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/effects", st.clone(), list_effects);
    r.post("/effects", st.clone(), create_effect);
    r.get("/effects/{id}", st.clone(), get_effect);
    r.put("/effects/{id}", st.clone(), update_effect);
    r.delete("/effects/{id}", st.clone(), delete_effect);
}
