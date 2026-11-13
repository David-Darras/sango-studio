//! Textures that a page forgets while the frame may still draw them.
//!
//! A page keeps its textures in a map and throws one away when its picture changes (an import, a reset). If the texture was drawn
//! earlier in the same frame, dropping the last handle destroys it before the graphics card has drawn the frame, and the program
//! stops ("Texture has been destroyed"). [`Retired`] keeps the handle alive for a few more frames.

use eframe::egui;
use std::collections::HashMap;
use std::hash::Hash;

/// How many frames a forgotten texture lives on.
const FRAMES: u64 = 4;

/// The textures that were forgotten and are kept for a few frames.
#[derive(Default)]
pub struct Retired {
    frame: u64,
    held: Vec<(u64, egui::TextureHandle)>,
}

impl Retired {
    /// Tells the frame that is drawn: call it each time that the page makes a texture. The handles that are old enough are dropped.
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.frame = ctx.cumulative_pass_nr();
        let now = self.frame;
        self.held.retain(|(since, _)| now.saturating_sub(*since) < FRAMES);
    }

    /// Keeps a handle for a few frames.
    pub fn keep(&mut self, handle: egui::TextureHandle) {
        self.held.push((self.frame, handle));
    }

    /// Forgets the texture of a key of a map: it is kept for a few frames.
    pub fn forget<K: Hash + Eq>(&mut self, map: &mut HashMap<K, Option<egui::TextureHandle>>, key: &K) {
        if let Some(Some(handle)) = map.remove(key) {
            self.keep(handle);
        }
    }

    /// Forgets all the textures of a map.
    pub fn forget_all<K: Hash + Eq>(&mut self, map: &mut HashMap<K, Option<egui::TextureHandle>>) {
        for (_, handle) in map.drain() {
            if let Some(handle) = handle {
                self.keep(handle);
            }
        }
    }
}
