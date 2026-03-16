use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "iss";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "ISS",
        icon: egui_phosphor::fill::ROCKET,
        color: Color32::YELLOW,
        icon_size: 20.0,
        use_bearing: false,
        icon_angle_offset: 0.0,
    }
}

#[derive(Deserialize, Debug)]
struct IssResponse {
    latitude: f64,
    longitude: f64,
}

pub async fn fetch() -> Option<Vec<ActorUpdate>> {
    let resp = reqwest::get("https://api.wheretheiss.at/v1/satellites/25544")
        .await
        .ok()?
        .json::<IssResponse>()
        .await;

    match resp {
        Ok(data) => Some(vec![ActorUpdate {
            id: "ISS".into(),
            label: "ISS".into(),
            lat: data.latitude,
            lon: data.longitude,
            bearing: None,
            speed: None,
            style: style(),
            source_tag: TAG,
        }]),
        Err(e) => {
            warn!("ISS fetch failed: {e}");
            None
        }
    }
}
