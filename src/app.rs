use std::collections::HashSet;
use std::sync::mpsc;
use web_time::Instant;

use bevy_ecs::prelude::*;
use egui::{
    epaint::TextShape, vec2, Align2, CentralPanel, Color32, FontId, RichText, Sense, Window,
};
use walkers::sources::{Attribution, TileSource};
use walkers::{lat_lon, Map, MapMemory, Projector, TileId};

use crate::components::*;
use crate::sources;
use crate::sources::ActorUpdate;

/// Convert a colorous::Color to egui Color32
fn colorous_to_egui(c: colorous::Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Registered source for the toggle menu
struct SourceEntry {
    tag: &'static str,
    style: SourceStyle,
}

pub struct TerrarApp {
    pub tiles: walkers::HttpTiles,
    pub map_memory: MapMemory,
    pub world: World,
    pub rx: mpsc::Receiver<Vec<ActorUpdate>>,
    pub last_zoom_time: Instant,
    pub disabled_sources: HashSet<&'static str>,
    known_sources: Vec<SourceEntry>,
}

/// Light monochrome tiles from CartoDB (Positron)
struct CartoPositron;

impl TileSource for CartoPositron {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://basemaps.cartocdn.com/light_all/{}/{}/{}.png",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "CartoDB / OpenStreetMap contributors",
            url: "https://carto.com/attributions",
            logo_light: None,
            logo_dark: None,
        }
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Fill);
    ctx.set_fonts(fonts);
}

impl TerrarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: mpsc::Receiver<Vec<ActorUpdate>>) -> Self {
        setup_fonts(&cc.egui_ctx);

        let mut map_memory = MapMemory::default();
        map_memory.center_at(lat_lon(50.0, 10.0));
        for _ in 0..4 {
            let _ = map_memory.zoom_out();
        }

        #[cfg(not(target_arch = "wasm32"))]
        let tiles = walkers::HttpTiles::with_options(
            CartoPositron,
            walkers::HttpOptions {
                cache: Some(std::path::PathBuf::from("tilecache")),
                ..Default::default()
            },
            cc.egui_ctx.clone(),
        );
        #[cfg(target_arch = "wasm32")]
        let tiles = walkers::HttpTiles::new(CartoPositron, cc.egui_ctx.clone());

        // Assign colors from TABLEAU10 to each source
        let palette = colorous::TABLEAU10;
        let source_defs: Vec<(&str, SourceStyle)> = vec![
            (sources::iss::TAG, sources::iss::style()),
            (sources::opensky::TAG, sources::opensky::style()),
            (sources::ships::TAG, sources::ships::style()),
            (sources::dwd::TAG, sources::dwd::style()),
            (sources::vbb::TAG, sources::vbb::style()),
            (sources::autobahn::TAG, sources::autobahn::style()),
        ];

        let known_sources = source_defs
            .into_iter()
            .enumerate()
            .map(|(i, (tag, mut style))| {
                style.color = colorous_to_egui(palette[i % palette.len()]);
                SourceEntry { tag, style }
            })
            .collect();

        Self {
            tiles,
            map_memory,
            world: World::new(),
            rx,
            last_zoom_time: Instant::now(),
            disabled_sources: HashSet::new(),
            known_sources,
        }
    }

    fn ingest_updates(&mut self) {
        // Build a map of source tag -> assigned color so we override incoming styles
        let color_map: std::collections::HashMap<&str, Color32> = self
            .known_sources
            .iter()
            .map(|s| (s.tag, s.style.color))
            .collect();

        while let Ok(updates) = self.rx.try_recv() {
            for mut update in updates {
                // Override color with the palette-assigned color for this source
                if let Some(&color) = color_map.get(update.source_tag) {
                    update.style.color = color;
                }

                let existing = self
                    .world
                    .query::<(Entity, &SourceId, &SourceTag)>()
                    .iter(&self.world)
                    .find(|(_, id, tag)| id.0 == update.id && tag.0 == update.source_tag)
                    .map(|(e, _, _)| e);

                if let Some(entity) = existing {
                    if let Some(mut pos) = self.world.get_mut::<GeoPosition>(entity) {
                        pos.pt = geo::Point::new(update.lat, update.lon);
                        if let Some(b) = update.bearing {
                            pos.bearing = b.to_radians();
                        }
                    }
                    if let Some(spd) = update.speed {
                        if let Some(mut s) = self.world.get_mut::<Speed>(entity) {
                            s.0 = spd;
                        }
                    }
                    if let Some(mut label) = self.world.get_mut::<ActorLabel>(entity) {
                        label.0 = update.label;
                    }
                } else {
                    let bearing = update.bearing.unwrap_or(0.0).to_radians();
                    let mut e = self.world.spawn((
                        GeoPosition {
                            pt: geo::Point::new(update.lat, update.lon),
                            bearing,
                        },
                        ActorLabel(update.label),
                        SourceId(update.id),
                        SourceTag(update.source_tag),
                        update.style,
                    ));
                    if let Some(spd) = update.speed {
                        e.insert(Speed(spd));
                    }
                }
            }
        }
    }

    fn handle_scroll_zoom(&mut self, ctx: &egui::Context) {
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta.abs() < 1.0 {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_zoom_time).as_millis() < 150 {
            return;
        }
        self.last_zoom_time = now;

        let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
        let zoom_in = scroll_delta > 0.0;

        if let Some(pos) = pointer_pos {
            let screen_rect = ctx.content_rect();
            let projector = Projector::new(screen_rect, &self.map_memory, lat_lon(50.0, 10.0));
            let cursor_geo = projector.unproject(pos.to_vec2());

            if zoom_in {
                let _ = self.map_memory.zoom_in();
            } else {
                let _ = self.map_memory.zoom_out();
            }

            let projector_after =
                Projector::new(screen_rect, &self.map_memory, lat_lon(50.0, 10.0));
            let cursor_screen_after = projector_after.project(cursor_geo);
            let drift = pos.to_vec2() - cursor_screen_after;
            let new_center =
                projector_after.unproject(screen_rect.center().to_vec2() - drift);
            self.map_memory.center_at(new_center);
        } else if zoom_in {
            let _ = self.map_memory.zoom_in();
        } else {
            let _ = self.map_memory.zoom_out();
        }
    }
}

struct ActorView {
    pos: GeoPosition,
    label: ActorLabel,
    style: SourceStyle,
    id: SourceId,
    speed: Option<f64>,
}

impl eframe::App for TerrarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ingest_updates();
        self.handle_scroll_zoom(ctx);

        let actor_count = self
            .world
            .query::<&GeoPosition>()
            .iter(&self.world)
            .count();

        // Sources panel
        Window::new("Sources")
            .collapsible(true)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, [0., 30.])
            .show(ctx, |ui| {
                ui.label(format!("Actors: {actor_count}"));
                ui.separator();
                for source in &self.known_sources {
                    let enabled = !self.disabled_sources.contains(source.tag);
                    let icon = source.style.icon;
                    let label = format!("{} {}", icon, source.style.name);

                    let mut checked = enabled;
                    if ui
                        .checkbox(&mut checked, RichText::new(label).color(source.style.color))
                        .changed()
                    {
                        if checked {
                            self.disabled_sources.remove(source.tag);
                        } else {
                            self.disabled_sources.insert(source.tag);
                        }
                    }
                }
            });

        let my_position = lat_lon(50.0, 10.0);

        let actors: Vec<ActorView> = self
            .world
            .query::<(
                &GeoPosition,
                &ActorLabel,
                &SourceStyle,
                &SourceId,
                &SourceTag,
                Option<&Speed>,
            )>()
            .iter(&self.world)
            .filter(|(_, _, _, _, tag, _)| !self.disabled_sources.contains(tag.0))
            .map(|(pos, label, style, id, _, speed)| ActorView {
                pos: pos.clone(),
                label: label.clone(),
                style: style.clone(),
                id: id.clone(),
                speed: speed.map(|s| s.0),
            })
            .collect();

        CentralPanel::default().show(ctx, |ui| {
            Map::new(
                Some(&mut self.tiles),
                &mut self.map_memory,
                my_position,
            )
            .zoom_gesture(false)
            .panning(false)
            .show(ui, |ui, _response, projector, _memory| {
                for (i, actor) in actors.iter().enumerate() {
                    let screen_pos = projector
                        .project(lat_lon(actor.pos.pt.x(), actor.pos.pt.y()))
                        .to_pos2();

                    let galley = ui.painter().layout_no_wrap(
                        actor.style.icon.to_string(),
                        FontId::proportional(actor.style.icon_size),
                        actor.style.color,
                    );

                    let galley_size = galley.size();
                    let top_left = screen_pos - galley_size * 0.5;

                    let mut text_shape = TextShape::new(top_left, galley, actor.style.color);

                    if actor.style.use_bearing {
                        text_shape.angle =
                            actor.pos.bearing as f32 + actor.style.icon_angle_offset;
                    }

                    let visual_rect = text_shape.visual_bounding_rect();
                    ui.painter().add(text_shape);

                    let response =
                        ui.interact(visual_rect, egui::Id::new(("actor", i)), Sense::hover());

                    if response.hovered() {
                        ui.painter().text(
                            screen_pos + vec2(actor.style.icon_size * 0.7, 0.),
                            Align2::LEFT_CENTER,
                            &actor.label.0,
                            FontId::proportional(11.),
                            Color32::WHITE,
                        );

                        response.show_tooltip_ui(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    actor.style.color,
                                    RichText::new(actor.style.icon).size(16.),
                                );
                                ui.strong(actor.style.name);
                            });
                            ui.label(format!("ID: {}", actor.id.0));
                            ui.label(format!("Name: {}", actor.label.0));
                            ui.label(format!(
                                "Lat: {:.4}, Lon: {:.4}",
                                actor.pos.pt.x(),
                                actor.pos.pt.y()
                            ));
                            if let Some(speed) = actor.speed {
                                ui.label(format!("Speed: {:.0} km/h", speed));
                            }
                            ui.label(format!(
                                "Bearing: {:.0}\u{00B0}",
                                actor.pos.bearing.to_degrees()
                            ));
                        });
                    }
                }
            });

            Window::new("zoom")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::RIGHT_TOP, [-10., 10.])
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("+").heading()).clicked() {
                            let _ = self.map_memory.zoom_in();
                        }
                        if ui.button(RichText::new("-").heading()).clicked() {
                            let _ = self.map_memory.zoom_out();
                        }
                    });
                });
        });

        ctx.request_repaint();
    }
}
