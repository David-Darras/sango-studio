//! Textures that are never destroyed while a frame may still draw them.
//!
//! The graphics layer of the program destroys a texture as soon as the last handle of it is dropped, but the frame that is being drawn
//! may still use the texture: the program then stops with "Texture has been destroyed". It happens when a page forgets its pictures in
//! the frame that drew them (the files changed while the window was in the background, for example).
//!
//! Every texture that the pages make with [`load`] gets a second handle here. [`sweep`], called once for each frame, drops that handle
//! only when no page holds the texture any more and a few frames have passed.

use eframe::egui;
use std::sync::Mutex;

/// How many frames a texture that no page holds lives on.
const FRAMES: u32 = 6;

struct Held {
    handle: egui::TextureHandle,
    /// The number of frames since a page held the texture for the last time.
    idle: u32,
}

static HELD: Mutex<Vec<Held>> = Mutex::new(Vec::new());

/// Makes a texture, like `Context::load_texture`, and keeps it alive for a few frames after the page that made it lets it go.
pub fn load(ctx: &egui::Context, name: impl Into<String>, image: impl Into<egui::ImageData>, options: egui::TextureOptions) -> egui::TextureHandle {
    let handle = ctx.load_texture(name, image, options);
    if let Ok(mut held) = HELD.lock() {
        held.push(Held { handle: handle.clone(), idle: 0 });
    }
    handle
}

/// Call it once for each frame: lets go of the textures that nobody holds any more since a few frames.
pub fn sweep(ctx: &egui::Context) {
    let Ok(mut held) = HELD.lock() else { return };
    // The number of holders of each texture: two are the page and this list, one is this list alone. The lock of the manager is let go
    // before a handle can be dropped: dropping a handle takes the same lock.
    let holders: Vec<usize> = {
        let manager = ctx.tex_manager();
        let manager = manager.read();
        held.iter().map(|entry| manager.meta(entry.handle.id()).map_or(0, |meta| meta.retain_count)).collect()
    };
    let mut place = 0;
    held.retain_mut(|entry| {
        if holders[place] <= 1 {
            entry.idle += 1;
        } else {
            entry.idle = 0;
        }
        place += 1;
        entry.idle < FRAMES
    });
}

#[cfg(test)]
mod test {
    use super::*;

    fn image() -> egui::ColorImage {
        egui::ColorImage::new([2, 2], egui::Color32::WHITE)
    }

    #[test]
    fn a_texture_that_a_page_dropped_lives_on_for_a_few_frames() {
        let ctx = egui::Context::default();
        let handle = load(&ctx, "test_texture", image(), egui::TextureOptions::LINEAR);
        let id = handle.id();
        sweep(&ctx);
        drop(handle);
        for _ in 0..FRAMES - 1 {
            sweep(&ctx);
            assert!(ctx.tex_manager().read().meta(id).is_some(), "the texture is still there");
        }
        // The page holds nothing since FRAMES frames: the list lets go, and the texture is freed.
        sweep(&ctx);
        sweep(&ctx);
        assert!(ctx.tex_manager().read().meta(id).is_none(), "the texture is freed");
    }

    #[test]
    fn a_texture_that_a_page_holds_is_never_dropped() {
        let ctx = egui::Context::default();
        let handle = load(&ctx, "held_texture", image(), egui::TextureOptions::LINEAR);
        for _ in 0..FRAMES * 3 {
            sweep(&ctx);
        }
        assert!(ctx.tex_manager().read().meta(handle.id()).is_some());
    }
}
