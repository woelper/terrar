use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "dwd";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "Weather Warning",
        icon: egui_phosphor::fill::CLOUD_WARNING,
        color: Color32::from_rgb(255, 200, 0),
        icon_size: 16.0,
        use_bearing: false,
        icon_angle_offset: 0.0,
    }
}

#[derive(Deserialize, Debug)]
struct WfsResponse {
    features: Option<Vec<WfsFeature>>,
}

#[derive(Deserialize, Debug)]
struct WfsFeature {
    geometry: WfsGeometry,
    properties: WfsProperties,
}

#[derive(Deserialize, Debug)]
struct WfsGeometry {
    // MultiPolygon: [polygon][ring][point][coord]
    coordinates: Vec<Vec<Vec<Vec<f64>>>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
struct WfsProperties {
    identifier: Option<String>,
    name: Option<String>,
    headline: Option<String>,
    event: Option<String>,
    severity: Option<String>,
    ec_area_color: Option<String>,
}

fn parse_hex_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(200);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color32::from_rgb(r, g, b)
    } else {
        style().color
    }
}

/// Compute centroid of a MultiPolygon geometry
fn centroid(coords: &[Vec<Vec<Vec<f64>>>]) -> Option<(f64, f64)> {
    let mut sum_lon = 0.0;
    let mut sum_lat = 0.0;
    let mut count = 0usize;

    for polygon in coords {
        // Use the outer ring (first ring) of each polygon
        if let Some(ring) = polygon.first() {
            for point in ring {
                if point.len() >= 2 {
                    sum_lon += point[0];
                    sum_lat += point[1];
                    count += 1;
                }
            }
        }
    }

    if count > 0 {
        Some((sum_lat / count as f64, sum_lon / count as f64))
    } else {
        None
    }
}

/// Fetch DWD weather warnings for Germany via WFS GeoServer
pub async fn fetch() -> Option<Vec<ActorUpdate>> {
    let url = "https://maps.dwd.de/geoserver/dwd/ows?\
        service=WFS&version=2.0.0&request=GetFeature\
        &typeName=dwd:Warnungen_Gemeinden\
        &srsName=EPSG:4326\
        &outputFormat=application/json\
        &count=200";

    let resp = reqwest::get(url)
        .await
        .ok()?
        .json::<WfsResponse>()
        .await;

    let base_style = style();

    match resp {
        Ok(data) => {
            let features = data.features.unwrap_or_default();
            let mut updates = Vec::with_capacity(features.len());

            for feat in &features {
                let (lat, lon) = match centroid(&feat.geometry.coordinates) {
                    Some(c) => c,
                    None => continue,
                };

                let id = feat
                    .properties
                    .identifier
                    .clone()
                    .unwrap_or_else(|| format!("{:.4}_{:.4}", lat, lon));

                let event = feat.properties.event.as_deref().unwrap_or("Warning");
                let region = feat.properties.name.as_deref().unwrap_or("Unknown");
                let label = format!("{} - {}", event, region);

                // Use DWD's own color for this warning if available
                let color = feat
                    .properties
                    .ec_area_color
                    .as_deref()
                    .map(parse_hex_color)
                    .unwrap_or(base_style.color);

                let mut s = base_style.clone();
                s.color = color;

                updates.push(ActorUpdate {
                    id,
                    label,
                    lat,
                    lon,
                    bearing: None,
                    speed: None,
                    style: s,
                    source_tag: TAG,
                });
            }

            Some(updates)
        }
        Err(e) => {
            warn!("DWD fetch failed: {e}");
            None
        }
    }
}
