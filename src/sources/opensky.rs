use super::ActorUpdate;
use crate::components::SourceStyle;
use egui::Color32;
use log::warn;
use serde::Deserialize;

pub const TAG: &str = "opensky";

pub fn style() -> SourceStyle {
    SourceStyle {
        name: "Aircraft",
        icon: egui_phosphor::fill::AIRPLANE_TILT,
        color: Color32::DARK_RED,
        icon_size: 14.0,
        use_bearing: true,
        // AIRPLANE_TILT naturally points ~45deg (northeast)
        icon_angle_offset: -std::f32::consts::FRAC_PI_4,
    }
}

/// OpenSky category codes to human-readable names
fn category_name(cat: u64) -> &'static str {
    match cat {
        0 => "No info",
        1 => "No ADS-B",
        2 => "Light (<15500 lbs)",
        3 => "Small (15500-75000 lbs)",
        4 => "Large (75000-300000 lbs)",
        5 => "High vortex large",
        6 => "Heavy (>300000 lbs)",
        7 => "High performance",
        8 => "Rotorcraft",
        9 => "Glider/sailplane",
        10 => "Lighter-than-air",
        11 => "Parachutist/skydiver",
        12 => "Ultralight/hang-glider",
        14 => "UAV",
        15 => "Space vehicle",
        16 => "Surface emergency",
        17 => "Surface service",
        18 => "Obstruction",
        19 => "Cluster obstacle",
        20 => "Line obstacle",
        _ => "Unknown",
    }
}

#[derive(Deserialize, Debug)]
struct OpenSkyResponse {
    states: Option<Vec<Vec<serde_json::Value>>>,
}

/// Fetch aircraft in a bounding box (lat_min, lon_min, lat_max, lon_max).
/// State vector indices:
///  0: icao24, 1: callsign, 2: origin_country, 3: time_position, 4: last_contact
///  5: longitude, 6: latitude, 7: baro_altitude, 8: on_ground, 9: velocity (m/s)
/// 10: true_track (deg), 11: vertical_rate, 12: sensors, 13: geo_altitude
/// 14: squawk, 15: spi, 16: position_source/category
pub async fn fetch(bbox: (f64, f64, f64, f64)) -> Option<Vec<ActorUpdate>> {
    let url = format!(
        "https://opensky-network.org/api/states/all?lamin={}&lomin={}&lamax={}&lomax={}",
        bbox.0, bbox.1, bbox.2, bbox.3
    );

    let resp = reqwest::get(&url)
        .await
        .ok()?
        .json::<OpenSkyResponse>()
        .await;

    let s = style();

    match resp {
        Ok(data) => {
            let states = data.states.unwrap_or_default();
            let mut updates = Vec::with_capacity(states.len());

            for state in &states {
                let icao = state.first().and_then(|v| v.as_str()).unwrap_or("?");
                let callsign = state
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                let origin_country = state
                    .get(2)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let lon = state.get(5).and_then(|v| v.as_f64());
                let lat = state.get(6).and_then(|v| v.as_f64());
                let altitude = state.get(7).and_then(|v| v.as_f64());
                let velocity_ms = state.get(9).and_then(|v| v.as_f64());
                let track = state.get(10).and_then(|v| v.as_f64());
                let category = state.get(16).and_then(|v| v.as_u64()).unwrap_or(0);

                if let (Some(lat), Some(lon)) = (lat, lon) {
                    let cs = if callsign.is_empty() {
                        icao.to_string()
                    } else {
                        callsign.to_string()
                    };

                    let alt_str = altitude
                        .map(|a| format!(" {:.0}m", a))
                        .unwrap_or_default();

                    let label = if origin_country.is_empty() {
                        format!("{}{} [{}]", cs, alt_str, category_name(category))
                    } else {
                        format!(
                            "{} ({}){} [{}]",
                            cs,
                            origin_country,
                            alt_str,
                            category_name(category)
                        )
                    };

                    updates.push(ActorUpdate {
                        id: icao.to_string(),
                        label,
                        lat,
                        lon,
                        bearing: track,
                        speed: velocity_ms.map(|v| v * 3.6),
                        style: s.clone(),
                        source_tag: TAG,
                    });
                }
            }

            Some(updates)
        }
        Err(e) => {
            warn!("OpenSky fetch failed: {e}");
            None
        }
    }
}
