use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "autobahn";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "Autobahn",
        icon: egui_phosphor::fill::WARNING,
        color: Color32::WHITE, // overridden by colorous
        icon_size: 12.0,
        use_bearing: false,
        icon_angle_offset: 0.0,
    }
}

#[derive(Deserialize, Debug)]
struct RoadsResponse {
    roads: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct WarningResponse {
    warning: Option<Vec<AutobahnItem>>,
}

#[derive(Deserialize, Debug)]
struct RoadworksResponse {
    roadworks: Option<Vec<AutobahnItem>>,
}

#[derive(Deserialize, Debug)]
struct ClosureResponse {
    closure: Option<Vec<AutobahnItem>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AutobahnItem {
    identifier: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    display_type: Option<String>,
    coordinate: Option<Coordinate>,
    description: Option<Vec<String>>,
    is_blocked: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Coordinate {
    lat: LatLon,
    long: LatLon,
}

/// The API returns coordinates as either string or number
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum LatLon {
    Str(String),
    Num(f64),
}

impl LatLon {
    fn as_f64(&self) -> Option<f64> {
        match self {
            LatLon::Num(n) => Some(*n),
            LatLon::Str(s) => s.parse().ok(),
        }
    }
}

impl AutobahnItem {
    fn to_update(&self, kind: &str, s: &SourceStyle) -> Option<ActorUpdate> {
        let coord = self.coordinate.as_ref()?;
        let lat = coord.lat.as_f64()?;
        let lon = coord.long.as_f64()?;

        let id = self.identifier.as_deref().unwrap_or("?").to_string();
        let title = self.title.as_deref().unwrap_or("");
        let subtitle = self.subtitle.as_deref().unwrap_or("");
        let desc = self
            .description
            .as_ref()
            .map(|d| d.join(" "))
            .unwrap_or_default();

        let blocked = self.is_blocked.as_deref() == Some("true");

        let label = if subtitle.is_empty() {
            format!("[{}] {}", kind, title)
        } else {
            format!("[{}] {}{}", kind, title, subtitle)
        };

        let mut actor_style = s.clone();

        // Pick icon + tweak based on type
        match kind {
            "Warning" => {
                actor_style.icon = egui_phosphor::fill::WARNING;
                actor_style.icon_size = 14.0;
            }
            "Roadwork" => {
                actor_style.icon = egui_phosphor::fill::TRAFFIC_CONE;
                actor_style.icon_size = 10.0;
            }
            "Closure" => {
                actor_style.icon = if blocked {
                    egui_phosphor::fill::PROHIBIT
                } else {
                    egui_phosphor::fill::ROAD_HORIZON
                };
                actor_style.icon_size = 12.0;
            }
            _ => {}
        }

        Some(ActorUpdate {
            id,
            label,
            lat,
            lon,
            bearing: None,
            speed: None,
            style: actor_style,
            source_tag: TAG,
        })
    }
}

/// Fetch warnings, roadworks, and closures for a set of roads.
pub async fn fetch(roads: &[String]) -> Option<Vec<ActorUpdate>> {
    let client = reqwest::Client::new();
    let base = "https://verkehr.autobahn.de/o/autobahn";
    let s = style();
    let mut all_updates = Vec::new();

    for road in roads {
        // Warnings
        if let Ok(resp) = client
            .get(format!("{}/{}/services/warning", base, road))
            .send()
            .await
        {
            if let Ok(data) = resp.json::<WarningResponse>().await {
                for item in data.warning.unwrap_or_default() {
                    if let Some(u) = item.to_update("Warning", &s) {
                        all_updates.push(u);
                    }
                }
            }
        }

        // Roadworks
        if let Ok(resp) = client
            .get(format!("{}/{}/services/roadworks", base, road))
            .send()
            .await
        {
            if let Ok(data) = resp.json::<RoadworksResponse>().await {
                for item in data.roadworks.unwrap_or_default() {
                    if let Some(u) = item.to_update("Roadwork", &s) {
                        all_updates.push(u);
                    }
                }
            }
        }

        // Closures
        if let Ok(resp) = client
            .get(format!("{}/{}/services/closure", base, road))
            .send()
            .await
        {
            if let Ok(data) = resp.json::<ClosureResponse>().await {
                for item in data.closure.unwrap_or_default() {
                    if let Some(u) = item.to_update("Closure", &s) {
                        all_updates.push(u);
                    }
                }
            }
        }
    }

    if all_updates.is_empty() {
        None
    } else {
        Some(all_updates)
    }
}

/// Fetch the list of all Autobahn road IDs.
pub async fn fetch_roads() -> Vec<String> {
    let url = "https://verkehr.autobahn.de/o/autobahn/";
    match reqwest::get(url).await {
        Ok(resp) => match resp.json::<RoadsResponse>().await {
            Ok(data) => data.roads,
            Err(e) => {
                warn!("Failed to parse Autobahn roads: {e}");
                Vec::new()
            }
        },
        Err(e) => {
            warn!("Failed to fetch Autobahn roads: {e}");
            Vec::new()
        }
    }
}
