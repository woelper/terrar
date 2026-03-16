use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "vbb";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "Berlin Transit",
        icon: egui_phosphor::fill::TRAIN,
        color: Color32::WHITE, // overridden by colorous
        icon_size: 12.0,
        use_bearing: false,
        icon_angle_offset: 0.0,
    }
}

#[derive(Deserialize, Debug)]
struct RadarResponse {
    movements: Vec<Movement>,
}

#[derive(Deserialize, Debug)]
struct Movement {
    direction: Option<String>,
    location: Option<Location>,
    line: Option<Line>,
    #[serde(rename = "tripId")]
    trip_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Location {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize, Debug)]
struct Line {
    name: Option<String>,
    product: Option<String>,
}

/// Fetch live vehicle positions in the Berlin area via transport.rest
pub async fn fetch() -> Option<Vec<ActorUpdate>> {
    // Berlin bounding box
    let url = "https://v6.vbb.transport.rest/radar?\
        north=52.7&west=13.0&south=52.3&east=13.8&results=500&duration=30";

    let resp = reqwest::get(url)
        .await
        .ok()?
        .json::<RadarResponse>()
        .await;

    let s = style();

    match resp {
        Ok(data) => {
            let mut updates = Vec::with_capacity(data.movements.len());

            for mov in &data.movements {
                let loc = match &mov.location {
                    Some(l) => l,
                    None => continue,
                };
                let (lat, lon) = match (loc.latitude, loc.longitude) {
                    (Some(lat), Some(lon)) => (lat, lon),
                    _ => continue,
                };

                let trip_id = mov.trip_id.as_deref().unwrap_or("?");
                let line_name = mov
                    .line
                    .as_ref()
                    .and_then(|l| l.name.as_deref())
                    .unwrap_or("?");
                let product = mov
                    .line
                    .as_ref()
                    .and_then(|l| l.product.as_deref())
                    .unwrap_or("");
                let direction = mov.direction.as_deref().unwrap_or("");

                let label = format!("{} {}", line_name, direction);

                let icon = match product {
                    "suburban" => egui_phosphor::fill::TRAIN,
                    "subway" => egui_phosphor::fill::SUBWAY,
                    "tram" => egui_phosphor::fill::TRAM,
                    "bus" => egui_phosphor::fill::BUS,
                    "ferry" => egui_phosphor::fill::BOAT,
                    "express" | "regional" => egui_phosphor::fill::TRAIN,
                    _ => s.icon,
                };

                let mut actor_style = s.clone();
                actor_style.icon = icon;

                updates.push(ActorUpdate {
                    id: trip_id.to_string(),
                    label,
                    lat,
                    lon,
                    bearing: None,
                    speed: None,
                    style: actor_style,
                    source_tag: TAG,
                });
            }

            Some(updates)
        }
        Err(e) => {
            warn!("VBB fetch failed: {e}");
            None
        }
    }
}
