use domain::{BlendMode, ParamValue};
use httpd::{Request, Response, Router};
use serde::Deserialize;
use std::collections::HashMap;
use store::NewLayer;

use crate::ApiState;
use crate::error::ApiResult;
use crate::types::{LayerResponse, SceneResponse};

pub async fn get_active_scene(st: ApiState, _req: Request) -> ApiResult {
    let layers = st.store.get_active_layers().await?;
    Ok(Response::json(
        200,
        &layers
            .into_iter()
            .map(LayerResponse::from)
            .collect::<Vec<_>>(),
    ))
}

pub async fn clear_active_scene(st: ApiState, _req: Request) -> ApiResult {
    st.store.clear_active_scene().await?;
    st.runtime.halt();
    Ok(Response::empty(204))
}

#[derive(Deserialize)]
struct AddLayerRequest {
    effect_id: String,
    zone_id: String,
    #[serde(default)]
    blend_mode: BlendMode,
    #[serde(default)]
    params: HashMap<String, ParamValue>,
}

impl From<AddLayerRequest> for NewLayer {
    fn from(r: AddLayerRequest) -> Self {
        Self {
            effect_id: r.effect_id,
            zone_id: r.zone_id,
            blend_mode: r.blend_mode,
            params: r.params,
        }
    }
}

pub async fn set_active_scene(st: ApiState, req: Request) -> ApiResult {
    let body: Vec<AddLayerRequest> = req.json()?;
    let layers: Vec<NewLayer> = body.into_iter().map(Into::into).collect();
    st.store.replace_active_layers(&layers).await?;
    st.runtime.reload_active();
    let layers = st.store.get_active_layers().await?;
    Ok(Response::json(
        200,
        &layers
            .into_iter()
            .map(LayerResponse::from)
            .collect::<Vec<_>>(),
    ))
}

pub async fn add_layer(st: ApiState, req: Request) -> ApiResult {
    let body: AddLayerRequest = req.json()?;
    let layer = st
        .store
        .add_active_layer(
            &body.effect_id,
            &body.zone_id,
            body.blend_mode,
            &body.params,
        )
        .await?;
    st.runtime.reload_active();
    Ok(Response::json(201, &LayerResponse::from(layer)))
}

#[derive(Deserialize)]
struct PatchLayerRequest {
    zone_id: Option<String>,
    enabled: Option<bool>,
    blend_mode: Option<BlendMode>,
    params: Option<HashMap<String, ParamValue>>,
}

pub async fn patch_layer(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    let body: PatchLayerRequest = req.json()?;
    let current = st.store.get_active_layer(&id).await?;
    let zone_id = body.zone_id.as_deref().unwrap_or(&current.zone_id);
    let enabled = body.enabled.unwrap_or(current.enabled);
    let blend_mode = body.blend_mode.unwrap_or(current.blend_mode);
    let params = body.params.clone().unwrap_or(current.params);
    let layer = st
        .store
        .update_active_layer(&id, zone_id, enabled, blend_mode, &params)
        .await?;
    st.runtime.reload_active();
    Ok(Response::json(200, &LayerResponse::from(layer)))
}

pub async fn remove_layer(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    st.store.remove_active_layer(&id).await?;
    st.runtime.reload_active();
    Ok(Response::empty(204))
}

#[derive(Deserialize)]
struct ReorderRequest {
    ordered_ids: Vec<String>,
}

pub async fn reorder_layers(st: ApiState, req: Request) -> ApiResult {
    let body: ReorderRequest = req.json()?;
    st.store.reorder_active_layers(&body.ordered_ids).await?;
    st.runtime.reload_active();
    Ok(Response::empty(204))
}

#[derive(Deserialize)]
struct SaveSceneRequest {
    name: String,
}

pub async fn save_active_scene(st: ApiState, req: Request) -> ApiResult {
    let body: SaveSceneRequest = req.json()?;
    let scene = st.store.save_active_as_scene(&body.name).await?;
    Ok(Response::json(201, &SceneResponse::from(scene)))
}

pub async fn load_scene_into_active(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("scene_id").to_string();
    st.store.load_scene_into_active(&id).await?;
    st.runtime.reload_active();
    Ok(Response::empty(204))
}

pub async fn overwrite_scene_from_active(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("scene_id").to_string();
    st.store.overwrite_scene_from_active(&id).await?;
    Ok(Response::empty(204))
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/scenes/active", st.clone(), get_active_scene);
    r.put("/scenes/active", st.clone(), set_active_scene);
    r.delete("/scenes/active", st.clone(), clear_active_scene);
    r.post("/scenes/active/layers", st.clone(), add_layer);
    r.put("/scenes/active/layers/reorder", st.clone(), reorder_layers);
    r.patch("/scenes/active/layers/{id}", st.clone(), patch_layer);
    r.delete("/scenes/active/layers/{id}", st.clone(), remove_layer);
    r.post("/scenes/active/save", st.clone(), save_active_scene);
    r.post(
        "/scenes/active/load/{scene_id}",
        st.clone(),
        load_scene_into_active,
    );
    r.put(
        "/scenes/active/overwrite/{scene_id}",
        st.clone(),
        overwrite_scene_from_active,
    );
}
