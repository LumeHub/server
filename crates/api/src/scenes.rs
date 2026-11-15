use httpd::{Request, Response, Router};
use serde::{Deserialize, Serialize};

use crate::ApiState;
use crate::error::ApiResult;
use crate::types::{LayerResponse, SceneResponse};

#[derive(Serialize)]
struct SceneDetailResponse {
    id: String,
    name: String,
    layers: Vec<LayerResponse>,
}

pub async fn list_scenes(st: ApiState, _req: Request) -> ApiResult {
    let scenes = st.store.get_scenes().await?;
    Ok(Response::json(
        200,
        &scenes
            .into_iter()
            .map(SceneResponse::from)
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
struct SceneBody {
    name: String,
}

pub async fn create_scene(st: ApiState, req: Request) -> ApiResult {
    let body: SceneBody = req.json()?;
    let scene = st.store.create_scene(&body.name).await?;
    Ok(Response::json(201, &SceneResponse::from(scene)))
}

pub async fn get_scene(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    let scene = st.store.get_scene(&id).await?;
    let layers = st.store.get_layers(&id).await?;
    Ok(Response::json(
        200,
        &SceneDetailResponse {
            id: scene.id,
            name: scene.name,
            layers: layers.into_iter().map(LayerResponse::from).collect(),
        },
    ))
}

pub async fn update_scene(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    let body: SceneBody = req.json()?;
    let scene = st.store.update_scene(&id, &body.name).await?;
    Ok(Response::json(200, &SceneResponse::from(scene)))
}

pub async fn delete_scene(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    st.store.delete_scene(&id).await?;
    Ok(Response::empty(204))
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/scenes", st.clone(), list_scenes);
    r.post("/scenes", st.clone(), create_scene);
    r.get("/scenes/{id}", st.clone(), get_scene);
    r.put("/scenes/{id}", st.clone(), update_scene);
    r.delete("/scenes/{id}", st.clone(), delete_scene);
}
