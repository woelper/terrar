use bevy_ecs::prelude::*;
use egui::Color32;
use geo::Point;

/// Geographic position with bearing
#[derive(Component, Clone, Debug)]
pub struct GeoPosition {
    /// lat/lon as geo::Point
    pub pt: Point<f64>,
    /// Bearing in radians
    pub bearing: f64,
}

impl GeoPosition {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self {
            pt: Point::new(lat, lon),
            bearing: 0.0,
        }
    }
}

/// Speed in km/h
#[derive(Component, Clone, Debug)]
pub struct Speed(pub f64);

/// Display label for an actor
#[derive(Component, Clone, Debug)]
pub struct ActorLabel(pub String);

/// Visual style for a source — defined by each source module
#[derive(Component, Clone, Debug)]
pub struct SourceStyle {
    pub name: &'static str,
    pub icon: &'static str,
    pub color: Color32,
    pub icon_size: f32,
    pub use_bearing: bool,
    /// Offset in radians to align the icon's natural orientation with north
    pub icon_angle_offset: f32,
}

/// Unique ID from the source (e.g. ICAO24 hex for aircraft)
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceId(pub String);

/// Tag to group actors by source for upsert matching
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceTag(pub &'static str);
