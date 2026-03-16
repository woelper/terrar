pub mod autobahn;
pub mod dwd;
pub mod iss;
pub mod opensky;
pub mod ships;
pub mod vbb;

use crate::components::SourceStyle;

/// A position update from any data source
#[derive(Debug, Clone)]
pub struct ActorUpdate {
    /// Unique ID within the source
    pub id: String,
    pub label: String,
    pub lat: f64,
    pub lon: f64,
    /// Bearing in degrees (0 = north, clockwise)
    pub bearing: Option<f64>,
    /// Speed in km/h
    pub speed: Option<f64>,
    pub style: SourceStyle,
    /// Tag to identify the source for upsert matching
    pub source_tag: &'static str,
}
