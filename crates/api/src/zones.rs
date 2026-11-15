use httpd::{Request, Response, Router};
use serde::{Deserialize, Serialize};
use store::ZoneRecord;

use crate::ApiState;
use crate::error::{ApiError, ApiResult};

const ALL_ZONE_ID: &str = "all";

#[derive(Serialize)]
struct ZoneResponse {
    id: String,
    name: String,
    start_pixel: u32,
    end_pixel: u32,
    transition_length: u32,
}

impl From<ZoneRecord> for ZoneResponse {
    fn from(r: ZoneRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            start_pixel: r.start_pixel,
            end_pixel: r.end_pixel,
            transition_length: r.transition_length,
        }
    }
}

fn all_zone(strip_len: usize) -> ZoneResponse {
    ZoneResponse {
        id: ALL_ZONE_ID.to_string(),
        name: "Full Strip".to_string(),
        start_pixel: 0,
        end_pixel: strip_len.saturating_sub(1) as u32,
        transition_length: 0,
    }
}

pub async fn list_zones(st: ApiState, _req: Request) -> ApiResult {
    let mut zones = vec![all_zone(st.strip_len)];
    zones.extend(
        st.store
            .get_zones()
            .await?
            .into_iter()
            .map(ZoneResponse::from),
    );
    Ok(Response::json(200, &zones))
}

#[derive(Deserialize)]
struct ZoneBody {
    name: String,
    start_pixel: u32,
    end_pixel: u32,
    #[serde(default)]
    transition_length: u32,
}

pub async fn create_zone(st: ApiState, req: Request) -> ApiResult {
    let body: ZoneBody = req.json()?;
    let zone = st
        .store
        .create_zone(
            &body.name,
            body.start_pixel,
            body.end_pixel,
            body.transition_length,
        )
        .await?;
    Ok(Response::json(201, &ZoneResponse::from(zone)))
}

pub async fn get_zone(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    if id == ALL_ZONE_ID {
        return Ok(Response::json(200, &all_zone(st.strip_len)));
    }
    let zone = st.store.get_zone(&id).await?;
    Ok(Response::json(200, &ZoneResponse::from(zone)))
}

pub async fn update_zone(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    let body: ZoneBody = req.json()?;
    if id == ALL_ZONE_ID {
        return Err(ApiError::Forbidden("cannot modify the full-strip zone"));
    }
    let zone = st
        .store
        .update_zone(
            &id,
            &body.name,
            body.start_pixel,
            body.end_pixel,
            body.transition_length,
        )
        .await?;
    Ok(Response::json(200, &ZoneResponse::from(zone)))
}

pub async fn delete_zone(st: ApiState, req: Request) -> ApiResult {
    let id = req.param("id").to_string();
    if id == ALL_ZONE_ID {
        return Err(ApiError::Forbidden("cannot delete the full-strip zone"));
    }
    st.store.delete_zone(&id).await?;
    Ok(Response::empty(204))
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/zones", st.clone(), list_zones);
    r.post("/zones", st.clone(), create_zone);
    r.get("/zones/{id}", st.clone(), get_zone);
    r.put("/zones/{id}", st.clone(), update_zone);
    r.delete("/zones/{id}", st.clone(), delete_zone);
}
