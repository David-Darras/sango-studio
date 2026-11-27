//! The view of a 3D model of a pack: the new model, and the model that it replaces when it replaces one (before and after).
//!
//! The two models are drawn on the processor (see `sango_core::render`) with the same point of view: the user turns one with the
//! mouse and the other turns with it, so that the two can be compared.

use sango_core::model::{Model, Texture};
use sango_core::render::{render, Camera};
use sango_core::{Dump, Garc};
use sango_pack::Pack;
use sango_ui::theme::color;
use sango_ui::{egui, retired::Retired, textures, widget};
use std::collections::HashMap;

/// The side of a picture, in pixels.
const SIZE: usize = 300;
/// The number of the archive of the models of the Pokémon.
const MODEL_ARCHIVE: usize = 8;

/// One model that is ready to be drawn.
struct Shown {
    model: Model,
    textures: HashMap<String, Texture>,
    picture: Option<egui::TextureHandle>,
}

impl Shown {
    fn new(model: Model, textures: Vec<Texture>) -> Shown {
        Shown { model, textures: textures.into_iter().map(|t| (t.name.clone(), t)).collect(), picture: None }
    }
}

/// What the view shows: a model of the pack, and maybe the model of the game that it replaces.
#[derive(Default)]
pub struct ModelPair {
    /// The name of the model in the pack.
    pub open: Option<String>,
    after: Option<Shown>,
    before: Option<Shown>,
    /// Why the new model is not shown.
    error: Option<String>,
    /// Why the old model is not shown.
    before_error: Option<String>,
    camera: Camera,
    dirty: bool,
    retired: Retired,
}

impl ModelPair {
    pub fn close(&mut self) {
        self.open = None;
        for shown in [self.after.take(), self.before.take()].into_iter().flatten() {
            if let Some(picture) = shown.picture {
                self.retired.keep(picture);
            }
        }
        self.error = None;
        self.before_error = None;
    }

    /// Reads the models. `replaces` is the species of the game and the slot that the model replaces, if it does.
    pub fn select(&mut self, pack: &Pack, name: &str, replaces: Option<(u16, u8)>, dump: Option<&Dump>) {
        self.close();
        self.open = Some(name.to_string());
        self.camera = Camera::default();
        self.dirty = true;
        match pack.read_model(name).and_then(|files| sango_species::read_pack_model(&files)) {
            Ok((model, textures)) => self.after = Some(Shown::new(model, textures)),
            Err(message) => self.error = Some(message),
        }
        if let Some((species, slot)) = replaces {
            let loaded = dump.ok_or_else(|| "The dump of the game is needed to show the old model.".to_string()).and_then(|dump| {
                let garc = Garc::open(&dump.archive(MODEL_ARCHIVE))?;
                sango_species::read_game_model(&garc, species as usize, slot)
            });
            match loaded {
                Ok((model, textures)) => self.before = Some(Shown::new(model, textures)),
                Err(message) => self.before_error = Some(message),
            }
        }
    }

    /// Draws one of the two pictures. Returns what the mouse did on it.
    fn draw(&mut self, ui: &mut egui::Ui, after: bool, title: &str) -> egui::Response {
        let ctx = ui.ctx().clone();
        let camera = self.camera;
        let dirty = self.dirty;
        let retired = &mut self.retired;
        let shown = if after { self.after.as_mut() } else { self.before.as_mut() };
        ui.vertical(|ui| {
            ui.label(widget::hint(title));
            let (rect, answer) = ui.allocate_exact_size(egui::vec2(SIZE as f32, SIZE as f32), egui::Sense::click_and_drag());
            if let Some(shown) = shown {
                if dirty || shown.picture.is_none() {
                    let pixels = render(&shown.model, &shown.textures, &camera, SIZE, SIZE);
                    let image = egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &pixels);
                    if let Some(old) = shown.picture.take() {
                        retired.keep(old);
                    }
                    shown.picture = Some(textures::load(&ctx, format!("pack_model_{after}"), image, egui::TextureOptions::LINEAR));
                }
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
                if let Some(picture) = &shown.picture {
                    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(picture.id(), rect, full, egui::Color32::WHITE);
                }
            } else {
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE_STRONG);
            }
            answer
        })
        .inner
    }

    /// Draws the view of the open model.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.retired.tick(ui.ctx());
        if self.open.is_none() {
            return;
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        let has_before = self.before.is_some();
        let mut dragged = egui::Vec2::ZERO;
        let mut scrolled = 0.0;
        ui.horizontal(|ui| {
            if has_before {
                let answer = self.draw(ui, false, "Before: the model of the game");
                if answer.dragged() {
                    dragged += answer.drag_delta();
                }
                if answer.hovered() {
                    scrolled += ui.input(|i| i.smooth_scroll_delta.y);
                }
            }
            let title = if has_before { "After: the model of the pack" } else { "The model of the pack" };
            let answer = self.draw(ui, true, title);
            if answer.dragged() {
                dragged += answer.drag_delta();
            }
            if answer.hovered() {
                scrolled += ui.input(|i| i.smooth_scroll_delta.y);
            }
        });
        // The two pictures share the point of view.
        if dragged != egui::Vec2::ZERO {
            self.camera.yaw += dragged.x * 0.012;
            self.camera.pitch = (self.camera.pitch + dragged.y * 0.012).clamp(-1.4, 1.4);
            self.dirty = true;
        } else if scrolled != 0.0 {
            self.camera.zoom = (self.camera.zoom * (1.0 + scrolled * 0.002)).clamp(0.3, 6.0);
            self.dirty = true;
        } else {
            self.dirty = false;
        }
        if let Some(message) = &self.before_error {
            ui.label(egui::RichText::new(format!("The old model is not shown: {message}")).color(color::AMBER));
        }
        ui.label(widget::hint("Drag to turn the models, scroll to zoom. Both models turn together."));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the pack `packs/gen7.sangopack` (when the file is there): the model of a species is read and drawn. With the dump of the
    /// game (`SANGO_DUMP`), the model of a species of the game is read for the view "before" too.
    #[test]
    fn a_model_of_a_pack_is_drawn_and_a_model_of_the_game_comes_with_it() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/gen7.sangopack");
        if !file.exists() {
            return;
        }
        let pack = Pack::open(&file).unwrap();
        let name = pack.model_names().into_iter().next().expect("the pack has models");
        let dump = std::env::var("SANGO_DUMP").ok().and_then(|root| Dump::open(&root).ok());
        let mut view = ModelPair::default();
        view.select(&pack, &name, dump.as_ref().map(|_| (25u16, 0u8)), dump.as_ref());
        assert!(view.error.is_none(), "{:?}", view.error);
        assert!(view.after.is_some());
        if dump.is_some() {
            assert!(view.before.is_some(), "{:?}", view.before_error);
        }
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0)));
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| view.ui(ui));
        });
        assert!(view.after.as_ref().unwrap().picture.is_some());
    }
}
