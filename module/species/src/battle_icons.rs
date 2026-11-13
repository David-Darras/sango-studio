//! The icons of the types and of the categories of the moves, as the game draws them.
//!
//! The icons are pictures of 50 × 20 pixels in the layout archives of the game. A layout archive starts with the number
//! of pictures and one name for each picture; the icons of the types are the pictures `type_icon_NN_` of the archive 212,
//! the icons of the categories are the pictures `damage_type_icon_NN_` of the archive 93.

use sango_core::moves::Category;
use sango_core::{Clim, Dump, Garc};
use sango_ui::egui;
use std::sync::RwLock;

/// The archive of the layout that holds the icons of the types.
const TYPE_ARCHIVE: usize = 212;
/// The number of the English language in the archives of the game.
const ENGLISH: usize = 2;
/// The archive of the icons of the categories.
const CATEGORY_ARCHIVE: usize = 93;

/// A picture: 4 bytes for each pixel.
#[derive(Clone)]
struct Image {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

struct Icons {
    types: Vec<Option<Image>>,
    /// The icons of the types of the game in each language (the places of the languages in the archives), when the dump has them.
    types_by_language: Vec<Vec<Option<Image>>>,
    /// Status, Physical, Special.
    categories: Vec<Option<Image>>,
    /// Changes when the icons are read again, so that the pictures that the windows keep are made again.
    generation: u32,
}

static ICONS: RwLock<Option<Icons>> = RwLock::new(None);

/// The pictures of the first file of a layout archive (in English), with their names. The file starts with the number of pictures, then
/// one name of 64 bytes for each picture, then the pictures in the same order.
fn pictures(dump: &Dump, archive: usize) -> Vec<(String, Image)> {
    pictures_in(dump, archive, ENGLISH)
}

/// The same pictures, with the text of a language (the place of the language in the archives).
fn pictures_in(dump: &Dump, archive: usize, language: usize) -> Vec<(String, Image)> {
    let Ok(garc) = Garc::open(&dump.archive(archive)) else { return Vec::new() };
    // The files of a layout archive exist for each language of the game: the pictures have the text in English.
    let Some(bytes) = garc.file_or_first(0, language) else { return Vec::new() };
    let data = if sango_core::lz::is_compressed(bytes) { sango_core::lz::decompress(bytes).unwrap_or_default() } else { bytes.to_vec() };
    let count = data.get(..4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let Some(names) = data.get(4..4 + count * 64) else { return Vec::new() };
    let names = names.chunks(64).map(|n| String::from_utf8_lossy(&n[..n.iter().position(|&c| c == 0).unwrap_or(64)]).into_owned());
    // The pictures are found like the game does: each one ends with a footer that starts with "CLIM" and tells the size of
    // the picture. A picture that cannot be read keeps its place (the names follow the places).
    let body = &data[4 + count * 64..];
    let mut images = Vec::new();
    let mut at = 0;
    while at + 0x28 <= body.len() {
        if &body[at..at + 4] == b"CLIM" {
            let size = u32::from_le_bytes([body[at + 0xc], body[at + 0xd], body[at + 0xe], body[at + 0xf]]) as usize;
            let end = at + 0x28;
            let picture = (size <= end).then(|| Clim::parse(&body[end - size..end]).ok()).flatten().and_then(|clim| clim.to_rgba().ok().map(|rgba| Image { width: clim.width(), height: clim.height(), rgba }));
            images.push(picture);
        }
        at += 4;
    }
    names.zip(images).filter_map(|(name, image)| image.map(|i| (name, i))).collect()
}

/// The picture whose name starts with a text.
fn named(list: &[(String, Image)], start: &str) -> Option<Image> {
    list.iter().find(|(name, _)| name.starts_with(start)).map(|(_, image)| image.clone())
}

/// Reads the icons from the dump. Without them the pages draw coloured labels.
pub fn load(dump: &Dump) {
    let list = pictures(dump, TYPE_ARCHIVE);
    let types: Vec<Option<Image>> = (0..18).map(|k| named(&list, &format!("type_icon_{k:02}_"))).collect();
    let by_language: Vec<Vec<Option<Image>>> = (0..8)
        .map(|language| {
            let list = pictures_in(dump, TYPE_ARCHIVE, language);
            (0..18).map(|k| named(&list, &format!("type_icon_{k:02}_"))).collect()
        })
        .collect();
    let list = pictures(dump, CATEGORY_ARCHIVE);
    let categories: Vec<Option<Image>> = (0..3).map(|k| named(&list, &format!("damage_type_icon_{k:02}_"))).collect();
    let previous = ICONS.read().ok().and_then(|icons| icons.as_ref().map(|i| i.generation)).unwrap_or(0);
    if let Ok(mut icons) = ICONS.write() {
        let complete = |list: Vec<Option<Image>>| if list.iter().all(|i| i.is_some()) { list } else { Vec::new() };
        *icons = Some(Icons { types: complete(types), types_by_language: by_language.into_iter().map(|list| if list.iter().all(|i| i.is_some()) { list } else { Vec::new() }).collect(), categories: complete(categories), generation: previous + 1 });
    }
}

/// The picture of an icon as a texture of the window, made once.
fn texture(ctx: &egui::Context, kind: &str, number: usize, pick: impl Fn(&Icons) -> Option<Image>) -> Option<egui::TextureHandle> {
    let icons = ICONS.read().ok()?;
    let icons = icons.as_ref()?;
    let id = egui::Id::new(("battle_icon", kind, number, icons.generation));
    if let Some(handle) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(handle);
    }
    let image = pick(icons)?;
    let handle = ctx.load_texture(format!("battle_icon_{kind}_{number}"), egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.rgba), egui::TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

/// The new types of the user, with their icons, and a number that changes when the list changes.
struct CustomTypes {
    list: Vec<sango_core::custom_type::CustomType>,
    generation: u32,
    /// When the SD card was read for the last time.
    read_at: Option<std::time::Instant>,
}

static CUSTOM: RwLock<CustomTypes> = RwLock::new(CustomTypes { list: Vec::new(), generation: 0, read_at: None });

/// Reads the new types of the SD card again (the list `type/types.csv` and the icons), at most one time each second. The
/// menus of the types then show the new types after the 18 types of the game. Call it at the start of a page that has a menu
/// of types.
pub fn refresh_custom_types(sd: Option<&sango_core::SdCard>) {
    let due = CUSTOM.read().map_or(false, |c| c.read_at.map_or(true, |at| at.elapsed() >= std::time::Duration::from_secs(1)));
    if !due {
        return;
    }
    let list = sd.map(sango_core::custom_type::load).unwrap_or_default();
    if let Ok(mut custom) = CUSTOM.write() {
        custom.read_at = Some(std::time::Instant::now());
        if custom.list != list {
            custom.list = list;
            custom.generation += 1;
        }
    }
}

/// The numbers of the new types, in order.
pub fn custom_type_ids() -> Vec<u8> {
    CUSTOM.read().map(|c| c.list.iter().map(|t| t.id as u8).collect()).unwrap_or_default()
}

/// The English name of a new type, or nothing for a number that is not a new type.
pub fn custom_type_name(kind: u8) -> Option<String> {
    CUSTOM.read().ok()?.list.iter().find(|t| t.id == kind as usize).map(|t| t.name.clone())
}

/// The icon of a new type as a texture, when the SD card has its file.
fn custom_type_texture(ctx: &egui::Context, kind: u8) -> Option<egui::TextureHandle> {
    let custom = CUSTOM.read().ok()?;
    let id = egui::Id::new(("custom_type_icon", kind, custom.generation));
    if let Some(handle) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(handle);
    }
    let rgba = custom.list.iter().find(|t| t.id == kind as usize)?.icon.as_ref()?;
    let handle = ctx.load_texture(format!("custom_type_{kind}"), egui::ColorImage::from_rgba_unmultiplied([sango_core::custom_type::ICON_WIDTH, sango_core::custom_type::ICON_HEIGHT], rgba), egui::TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

/// The icon of a type (0 Normal to 17 Fairy, or the number of a new type), when it exists.
pub fn type_texture(ctx: &egui::Context, kind: u8) -> Option<egui::TextureHandle> {
    if kind as usize >= sango_core::custom_type::FIRST_ID {
        return custom_type_texture(ctx, kind);
    }
    texture(ctx, "type", kind as usize, |icons| icons.types.get(kind as usize).cloned().flatten())
}

/// The pixels of the icon of a type (50 × 20 pixels of RGBA), when the dump has it.
pub fn type_rgba(kind: u8) -> Option<(usize, usize, Vec<u8>)> {
    let icons = ICONS.read().ok()?;
    let image = icons.as_ref()?.types.get(kind as usize)?.as_ref()?;
    Some((image.width, image.height, image.rgba.clone()))
}

/// The pixels of the icon of a type of the game in a language (the place of the language in the archives): the icon has the
/// name of the type in that language. A language without icons gives the English icon.
pub fn type_rgba_in(kind: u8, language: usize) -> Option<(usize, usize, Vec<u8>)> {
    let icons = ICONS.read().ok()?;
    let icons = icons.as_ref()?;
    let image = icons.types_by_language.get(language).and_then(|list| list.get(kind as usize)).and_then(|image| image.as_ref()).or_else(|| icons.types.get(kind as usize)?.as_ref())?;
    Some((image.width, image.height, image.rgba.clone()))
}

/// The icon of a category of moves, when the dump has it.
pub fn category_texture(ctx: &egui::Context, category: Category) -> Option<egui::TextureHandle> {
    // The pictures of the archive are in this order: Status, Physical, Special.
    let place = match category {
        Category::Status => 0,
        Category::Physical => 1,
        Category::Special => 2,
    };
    texture(ctx, "category", place, |icons| icons.categories.get(place).cloned().flatten())
}

/// The size of an icon on the screen, in points.
pub const ICON_SIZE: egui::Vec2 = egui::vec2(50.0, 20.0);

/// Draws a texture at a place, and gives its rectangle.
pub fn paint(painter: &egui::Painter, texture: &egui::TextureHandle, left_center: egui::Pos2) -> egui::Rect {
    let rect = egui::Rect::from_min_size(egui::pos2(left_center.x, left_center.y - ICON_SIZE.y / 2.0), ICON_SIZE);
    painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    rect
}

/// Draws the icon of a type as the game has it. Gives its rectangle, or nothing without icon.
pub fn paint_type(painter: &egui::Painter, kind: u8, left_center: egui::Pos2) -> Option<egui::Rect> {
    let texture = type_texture(painter.ctx(), kind)?;
    Some(paint(painter, &texture, left_center))
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: each language has its own icons of the types, with the name of the type in that language.
    #[test]
    fn the_icons_of_the_types_exist_in_each_language() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        load(&Dump::open(&root).unwrap());
        for language in 0..8 {
            assert!(type_rgba_in(0, language).is_some(), "language {language}");
        }
        let english = type_rgba_in(0, ENGLISH).unwrap().2;
        let different = (0..8).filter(|&language| type_rgba_in(0, language).unwrap().2 != english).count();
        assert!(different > 0, "at least one language has an icon with another text");
    }

    /// With the dump of the game: the 18 icons of the types and the 3 icons of the categories are found.
    #[test]
    fn the_icons_of_the_dump_are_found() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        load(&dump);
        let icons = ICONS.read().unwrap();
        let icons = icons.as_ref().unwrap();
        assert_eq!(icons.types.len(), 18);
        assert!(icons.types.iter().all(|i| i.is_some()));
        assert!(icons.categories.len() >= 3 && icons.categories.iter().take(3).all(|i| i.is_some()));
        if let Ok(out) = std::env::var("SANGO_ICON_OUT") {
            for (k, icon) in icons.types.iter().enumerate() {
                let icon = icon.as_ref().unwrap();
                std::fs::write(format!("{out}/type_{k}.rgba"), &icon.rgba).unwrap();
            }
        }
        // The icons are not clear: they have pixels with colour.
        assert!(icons.types[9].as_ref().unwrap().rgba.chunks(4).filter(|p| p[3] > 200).count() > 400);
    }
}
