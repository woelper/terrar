use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "ships";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "Ship",
        icon: egui_phosphor::fill::ANCHOR,
        color: Color32::from_rgb(60, 200, 160),
        icon_size: 12.0,
        use_bearing: true,
        icon_angle_offset: 0.0,
    }
}

#[derive(Deserialize, Debug)]
struct AisResponse {
    features: Vec<AisFeature>,
}

#[derive(Deserialize, Debug)]
struct AisFeature {
    geometry: AisGeometry,
    properties: AisProperties,
}

#[derive(Deserialize, Debug)]
struct AisGeometry {
    coordinates: Vec<f64>, // [lon, lat]
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AisProperties {
    mmsi: u64,
    sog: Option<f64>,     // speed over ground in knots
    cog: Option<f64>,     // course over ground in degrees
    heading: Option<u32>, // heading in degrees
}

/// Fetch vessels near a point (lat, lon, radius_km).
pub async fn fetch(lat: f64, lon: f64, radius_km: f64) -> Option<Vec<ActorUpdate>> {
    let url = format!(
        "https://meri.digitraffic.fi/api/ais/v1/locations?latitude={}&longitude={}&radius={}",
        lat, lon, radius_km
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Digitraffic-User", "terrar")
        .send()
        .await
        .ok()?
        .json::<AisResponse>()
        .await;

    let s = style();

    match resp {
        Ok(data) => {
            let mut updates = Vec::with_capacity(data.features.len());

            for feat in &data.features {
                if feat.geometry.coordinates.len() < 2 {
                    continue;
                }
                let lon = feat.geometry.coordinates[0];
                let lat = feat.geometry.coordinates[1];
                let mmsi = feat.properties.mmsi;

                // Use heading if available, fall back to course over ground
                let bearing = feat
                    .properties
                    .heading
                    .filter(|&h| h < 360)
                    .map(|h| h as f64)
                    .or(feat.properties.cog);

                // Convert knots to km/h
                let speed = feat.properties.sog.map(|s| s * 1.852);

                updates.push(ActorUpdate {
                    id: mmsi.to_string(),
                    label: mmsi.to_string(),
                    lat,
                    lon,
                    bearing,
                    speed,
                    style: s.clone(),
                    source_tag: TAG,
                });
            }

            Some(updates)
        }
        Err(e) => {
            warn!("Ships fetch failed: {e}");
            None
        }
    }
}
