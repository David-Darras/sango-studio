//! The Overworld page of Sango Studio: the editor of the blocks, the tiles, the events and the models of the maps.
#![allow(dead_code)]

mod bch;
mod bch_edit;
mod catalog;
mod config;
mod data;
mod dict;
mod edit;
mod editor;
mod events;
mod garc;
mod icon;
mod models;
pub mod names;
mod plugin_dir;
mod scripts;
mod thumb;
mod viewer;

use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::{Path, PathBuf};

/// The key where the application keeps the graphics of the window, for the pages that draw in 3D.
pub const RENDER_STATE_KEY: &str = "sango_wgpu_render_state";

/// The Overworld page: the maps of the game, their ground, their objects, their events and their models.
#[derive(Default)]
pub struct Overworld {
    viewer: Option<viewer::Viewer>,
    /// What went wrong when the editor opened.
    error: Option<String>,
    /// The dump of the game that the editor was opened for.
    root: Option<PathBuf>,
    /// The frames that the page waited before it opened the editor: the page is drawn first, and the opening takes some seconds.
    waited: u32,
}

/// Opens the editor on the block that was open the last time.
fn open(state: &eframe::egui_wgpu::RenderState, root: &Path, sango: &Path, maps: &Path, own: &Path) -> Result<viewer::Viewer, String> {
    // The files of the maps are in the folder of the studio of the SD card, the files of the editor in the workspace.
    plugin_dir::set_maps(maps);
    config::set_folder(own);
    let arc = config::archives(Some(&root.to_string_lossy()))?;
    let block = config::load().block.unwrap_or(4);
    let scene = data::load_scene_from(&arc, block, None, None)?;
    let mut editor = editor::Editor::new(&arc, block)?;
    editor.set_tileset(scene.tileset);
    viewer::Viewer::new(state, sango, scene.textures, editor, format!("Block {block}"), arc)
}

impl Module for Overworld {
    fn title(&self) -> &str {
        "Overworld"
    }

    fn icon(&self) -> &str {
        "🗺"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        // The application draws the pages that are not open out of sight to load them: the editor opens when it is seen.
        if !ui.is_visible() {
            return;
        }
        let (Some(dump), Some(sd)) = (session.dump.as_ref(), session.sd.as_ref()) else {
            ui.heading("Overworld");
            ui.add_space(8.0);
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game and the SD card on the Home page."));
            return;
        };
        let root = dump.root().to_path_buf();
        scripts::refresh_created(sd);
        // The text files of the maps (the scripts, the events) stay in the folder `sango` of the SD card, next to the folder of the studio.
        let sango = sd.root().parent().map(|sdmc| sdmc.join("sango")).unwrap_or_else(|| PathBuf::from("sango"));
        let (maps, own) = (sd.overworld_folder(), sd.local().join("overworld"));
        if self.root.as_deref() != Some(root.as_path()) {
            *self = Overworld { root: Some(root.clone()), ..Overworld::default() };
        }
        if self.viewer.is_none() && self.error.is_none() {
            if self.waited < 2 {
                self.waited += 1;
                ui.heading("Overworld");
                ui.add_space(8.0);
                ui.label(widget::hint("Opening the map…"));
                ui.ctx().request_repaint();
                return;
            }
            let state = ui.ctx().data(|d| d.get_temp::<eframe::egui_wgpu::RenderState>(egui::Id::new(RENDER_STATE_KEY)));
            match state {
                Some(state) => match open(&state, &root, &sango, &maps, &own) {
                    Ok(viewer) => self.viewer = Some(viewer),
                    Err(message) => self.error = Some(message),
                },
                None => self.error = Some("The graphics card is not available.".into()),
            }
        }
        if let Some(message) = self.error.clone() {
            ui.heading("Overworld");
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(560.0_f32.min(ui.available_width()));
                ui.label(egui::RichText::new("The map did not open").color(color::ROSE).strong());
                ui.add_space(4.0);
                ui.label(widget::hint(&message));
                ui.add_space(8.0);
                if widget::button(ui, "Try again") {
                    self.error = None;
                    self.waited = 0;
                }
            });
            return;
        }
        if let Some(viewer) = self.viewer.as_mut() {
            viewer.ui(ui);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The dump of the game, when `SANGO_DUMP` gives it.
    fn dump() -> Option<String> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        config::archives(Some(&root)).ok()
    }

    /// With the dump: a block that the editor reads and writes again is the block of the game, byte for byte once decompressed.
    #[test]
    fn a_block_goes_through_the_editor_and_comes_back_identical() {
        let Some(arc) = dump() else { return };
        let files = garc::read(&garc::locate(&arc, garc::Archive::Blocks)).unwrap();
        for block in [4usize, 5, 42] {
            let original = files[block][0].clone();
            let expected = if original[0] == 0x11 { sango_core::lz::decompress(&original).unwrap() } else { original };
            let written = edit::EditableBlock::load(&arc, block).unwrap().to_bytes().unwrap();
            let again = if written[0] == 0x11 { sango_core::lz::decompress(&written).unwrap() } else { written };
            assert!(again == expected, "block {block} changed");
        }
    }

    /// With the dump: the pictures of a tileset are decoded (the size of each is its width, its height and 4 bytes).
    #[test]
    fn the_pictures_of_a_block_are_decoded() {
        let Some(arc) = dump() else { return };
        let scene = data::load_scene_from(&arc, 4, None, None).unwrap();
        assert!(!scene.meshes.is_empty() && !scene.textures.is_empty());
        for texture in scene.textures.values() {
            assert_eq!(texture.rgba.len(), texture.width * texture.height * 4, "{}", texture.name);
        }
    }

    /// With the dump: the events of the maps are read and written again like the game has them.
    #[test]
    fn the_events_of_the_maps_are_written_again_like_the_game_has_them() {
        let Some(arc) = dump() else { return };
        let maps = garc::read(&garc::locate(&arc, garc::Archive::Maps)).unwrap();
        let (mut same, mut read) = (0, 0);
        for file in &maps {
            let Some(raw) = file.first() else { continue };
            let data = if raw[0] == 0x11 { sango_core::lz::decompress(raw).unwrap() } else { raw.clone() };
            let Ok((_, parts)) = sango_core::bundle::split(&data) else { continue };
            let Some(part) = parts.get(1) else { continue };
            let Ok(events) = events::MapEvents::parse(part) else { continue };
            read += 1;
            let rebuilt = events.to_bytes();
            let common = part.len().min(rebuilt.len());
            if rebuilt[..common] == part[..common] && rebuilt.len() - common < 4 {
                same += 1;
            }
        }
        assert!(read > 100 && same == read, "{same} of {read} maps are identical");
    }

    /// The list of a folder: a file is added, and taken away again.
    #[test]
    fn the_list_of_a_folder_follows_its_files() {
        let folder = std::env::temp_dir().join(format!("sango_overworld_index_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        edit::write_index(&folder, 7, 100).unwrap();
        edit::write_index(&folder, 3, 50).unwrap();
        assert_eq!(sango_core::file_lists::parse_index(&std::fs::read(folder.join("index.bin")).unwrap()).unwrap(), vec![(3, 50), (7, 100)]);
        edit::remove_index(&folder, 3).unwrap();
        edit::remove_index(&folder, 7).unwrap();
        assert!(!folder.join("index.bin").exists(), "a folder without a file has no list");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The tree of the names of a model has the node of the root and one node for each name, in the form of the file.
    #[test]
    fn the_dictionary_of_a_model_has_one_node_for_each_name() {
        let names: Vec<&[u8]> = vec![b"BodyA", b"Eye", b"Mouth"];
        let nodes = dict::build(&names, &[0, 6, 10]);
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].bit, u32::MAX);
        assert_eq!(dict::Node::from_bytes(&nodes[2].to_bytes()), nodes[2]);
        assert_eq!(nodes[1].name, 0);
        assert_eq!(nodes[3].name, 10);
    }
}
