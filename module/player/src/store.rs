//! The files of the player characters, and how the studio changes them.
//!
//! | What | Archive of the game | File on the SD card |
//! | --- | --- | --- |
//! | PSS icons: BCLIM, 40 × 40, with a palette, LZ11 | 108, one file for each icon | `player/icon/<file>.bin` |
//! | Pictures of a menu (Town Map, Soar map): a BCLIM in a layout bundle | 67 and 255 | `layout/<archive>/<file>.bin` |
//! | Portraits of the choice of the character: BCLIM, 220 × 220, in a layout bundle, the first part of a pack | 269 | `player/intro/0.bin` |
//! | Overworld models: a pack (model, motions) | 21 | `overworld/model/<pack>.bin` |
//! | Soar models: a pack (motions, then the model) | 254 | `player/soaring_model/<pack>.bin` |
//! | Battle models: a BCH file | 133, five files for each model | `trainer/model/<file>.bin` |
//!
//! The plugin gives these files to the game in place of the files of the archives: restart the game after a change.

use sango_core::model::{parse_model, parse_textures};
use sango_core::{bundle, lz, Clim, Dump, Garc, SdCard};
use sango_layout::bundle::Bundle;
use sango_modelport::skeleton::Joint;
use std::path::Path;

/// The archive of the PSS icons.
pub const ICON_ARCHIVE: usize = 108;
/// The archive of the models of the player in the Soar sequence.
const SOARING_ARCHIVE: usize = 254;
/// The archive of the introduction of the game: one pack, and its first part is the bundle of the menu of the introduction.
const INTRO_ARCHIVE: usize = 269;
/// The number of things of a character.
pub const ENTRY_COUNT: usize = 12;
/// The side of a PSS icon, in pixels.
pub const ICON_SIDE: usize = 40;
/// The number of files of each battle model.
const BATTLE_FILES: usize = 5;
/// The alignment of the parts in a pack of models.
const ALIGNMENT: usize = 128;

/// The two player characters.
pub const PLAYERS: [&str; 2] = ["Brendan", "May"];

/// The models of the battles of the game, by their number. The number 3 is a mark of the developers, not a character.
const BATTLE_MODELS: [(u8, &str); 14] = [
    (4, "May"), (5, "Brendan"), (6, "Wally"), (7, "Wally (2)"), (8, "Steven"), (9, "Maxie"), (10, "Archie"), (11, "Zinnia"),
    (0, "Serena (X and Y)"), (1, "Calem (X and Y)"), (2, "Malva (X and Y)"), (12, "Shauna (X and Y)"), (13, "Tierno (X and Y)"), (14, "Trevor (X and Y)"),
];

/// What a thing of a player character is, and where it is in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A PSS icon: a file of the archive of the icons.
    Icon(usize),
    /// A picture of a menu: the archive, the file (a layout bundle) and the name of the picture in it.
    Picture(usize, usize, &'static str),
    /// A picture of the menu of the introduction: its name in the bundle.
    Intro(&'static str),
    /// A model of the overworld: a pack of the archive of the character models.
    Overworld(usize),
    /// The model of the Soar sequence: a pack.
    Soaring(usize),
    /// A model of the battles: its number.
    Battle(u8),
}

impl Kind {
    pub fn is_model(self) -> bool {
        matches!(self, Kind::Overworld(_) | Kind::Soaring(_) | Kind::Battle(_))
    }

    /// The file on the SD card that replaces the file of the game.
    pub fn path(self) -> String {
        match self {
            Kind::Icon(file) => format!("player/icon/{file}.bin"),
            Kind::Picture(archive, file, _) => format!("layout/{archive}/{file}.bin"),
            Kind::Intro(_) => "player/intro/0.bin".to_string(),
            Kind::Overworld(pack) => format!("overworld/model/{pack}.bin"),
            Kind::Soaring(pack) => format!("player/soaring_model/{pack}.bin"),
            Kind::Battle(number) => format!("trainer/model/{}.bin", number as usize * BATTLE_FILES),
        }
    }
}

/// One thing of the list of a player character.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: &'static str,
    /// Where the game shows it.
    pub note: &'static str,
    pub kind: Kind,
}

/// The things of a player character (0 Brendan, 1 May): the pictures first, then the models.
pub fn entries(player: usize) -> [Entry; ENTRY_COUNT] {
    let pick = |brendan: usize, may: usize| if player == 0 { brendan } else { may };
    let head = if player == 0 { "hero_obj.bclim" } else { "heroine_obj.bclim" };
    // The game writes "famale" in the name of the file.
    let portrait = if player == 0 { "intro_male.bclim" } else { "intro_famale.bclim" };
    let entry = |name, note, kind| Entry { name, note, kind };
    [
        entry("PSS icon", "The face of the player in the PSS, on the title screen, on the name screen and before a Link Battle.", Kind::Icon(pick(38, 39))),
        entry("Selection portrait", "The portrait that the player chooses at the start of a new game.", Kind::Intro(portrait)),
        entry("Town Map head", "The head that marks the place of the player on the Town Map.", Kind::Picture(67, 0, head)),
        entry("Town Map shadow", "The shadow under the head on the Town Map. Brendan and May use the same one.", Kind::Picture(67, 0, "hero_shadow.bclim")),
        entry("Soar map arrow", "The mark of the player on the map of the Soar sequence. Brendan and May use the same one.", Kind::Picture(255, 0, "hero.bclim")),
        entry("Overworld", "The character that walks on the maps.", Kind::Overworld(pick(172, 171))),
        entry("Overworld, contest costume", "The character in a Pokémon Contest.", Kind::Overworld(pick(174, 173))),
        entry("Overworld, Magma suit", "The character in the suit of Team Magma.", Kind::Overworld(pick(177, 175))),
        entry("Overworld, Aqua suit", "The character in the suit of Team Aqua.", Kind::Overworld(pick(178, 176))),
        entry("Overworld, partner", "This character as the other trainer who follows you.", Kind::Overworld(pick(315, 314))),
        entry("Soaring", "The rider on Latios or Latias in the Soar sequence.", Kind::Soaring(pick(3, 2))),
        entry("Battle", "The trainer in a battle (also with the Mega Bracelet).", Kind::Battle(pick(5, 4) as u8)),
    ]
}

fn plain(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if lz::is_compressed(&bytes) {
        lz::decompress(&bytes)
    } else {
        Ok(bytes)
    }
}

/// The part of a pack that holds the 3D model.
fn model_part(parts: &[&[u8]]) -> Result<usize, String> {
    parts.iter().position(|part| parse_model(part).is_ok_and(|model| !model.meshes.is_empty())).ok_or_else(|| "The pack has no model.".to_string())
}

/// The archives of the dump.
pub struct Store {
    icons: Garc,
    soaring: Garc,
    dump: Dump,
    characters: sango_character::store::Store,
    trainers: sango_trainer::store::Store,
}

impl Store {
    pub fn open(dump: &Dump) -> Result<Store, String> {
        Ok(Store {
            icons: Garc::open(&dump.archive(ICON_ARCHIVE))?,
            soaring: Garc::open(&dump.archive(SOARING_ARCHIVE))?,
            dump: Dump::open(&dump.root().to_string_lossy())?,
            characters: sango_character::store::Store::open(dump)?,
            trainers: sango_trainer::store::Store::open(dump)?,
        })
    }

    /// The number of PSS icons (the files 1 to this number).
    pub fn icon_count(&self) -> usize {
        self.icons.file_count().saturating_sub(1)
    }

    /// The file of the game that holds a thing, as the archive has it.
    fn game_file(&self, kind: Kind) -> Result<Vec<u8>, String> {
        let missing = || "This file is not in the archive of the game.".to_string();
        match kind {
            Kind::Icon(file) => self.icons.first_file(file).map(<[u8]>::to_vec).ok_or_else(missing),
            Kind::Picture(archive, file, _) => Garc::open(&self.dump.archive(archive))?.first_file(file).map(<[u8]>::to_vec).ok_or_else(missing),
            Kind::Overworld(pack) => self.characters.game_pack(pack).ok_or_else(missing),
            Kind::Intro(_) => Garc::open(&self.dump.archive(INTRO_ARCHIVE))?.first_file(0).map(<[u8]>::to_vec).ok_or_else(missing),
            Kind::Soaring(pack) => self.soaring.first_file_decompressed(pack).ok_or_else(missing),
            Kind::Battle(number) => self.trainers.game_model_file(number).ok_or_else(missing),
        }
    }

    /// The file that the game uses now: the one of the SD card, else the one of the game.
    fn current_file(&self, sd: Option<&SdCard>, kind: Kind) -> Result<Vec<u8>, String> {
        match sd.and_then(|sd| sd.read(&kind.path())) {
            Some(bytes) => Ok(bytes),
            None => self.game_file(kind),
        }
    }

    /// The layout bundle of a file: the file itself, or the first part of the pack of the introduction.
    fn bundle_in(kind: Kind, file: &[u8]) -> Result<Bundle, String> {
        match kind {
            Kind::Intro(_) => Bundle::parse(bundle::split(file)?.1.first().ok_or("The pack of the introduction is empty.")?),
            _ => Bundle::parse(file),
        }
    }

    /// The file with another layout bundle.
    fn file_with(kind: Kind, file: &[u8], menu: &Bundle) -> Result<Vec<u8>, String> {
        let mut bytes = menu.to_bytes();
        match kind {
            Kind::Intro(_) => {
                // A part of a pack ends at a multiple of its alignment.
                bytes.resize(bytes.len().next_multiple_of(ALIGNMENT), 0);
                bundle::replace_part_with(file, 0, &bytes, ALIGNMENT)
            }
            _ => Ok(bytes),
        }
    }

    /// The BCLIM image of a picture in a file (an icon file, or a layout bundle).
    fn image_in(kind: Kind, file: Vec<u8>) -> Result<Vec<u8>, String> {
        match kind {
            Kind::Picture(_, _, name) | Kind::Intro(name) => {
                let menu = Store::bundle_in(kind, &file)?;
                menu.darc.files.iter().find(|f| f.path.rsplit('/').next() == Some(name)).map(|f| f.data.clone()).ok_or_else(|| format!("The menu file has no picture {name}."))
            }
            Kind::Icon(_) => plain(file),
            _ => Err("This is not a picture.".to_string()),
        }
    }

    /// A picture: its size and its RGBA pixels.
    pub fn picture(&self, sd: Option<&SdCard>, kind: Kind) -> Result<(usize, usize, Vec<u8>), String> {
        let picture = sango_layout::tex::decode(&Store::image_in(kind, self.current_file(sd, kind)?)?)?;
        Ok((picture.width, picture.height, picture.rgba))
    }

    /// True when the SD card changes this thing.
    pub fn changed(&self, sd: Option<&SdCard>, kind: Kind) -> bool {
        let Some(bytes) = sd.and_then(|sd| sd.read(&kind.path())) else { return false };
        match kind {
            // A menu file holds many pictures (and the Layout page changes them too): only this picture counts.
            Kind::Picture(..) | Kind::Intro(_) => Store::image_in(kind, bytes).ok() != self.game_file(kind).and_then(|file| Store::image_in(kind, file)).ok(),
            _ => true,
        }
    }

    fn write(sd: &SdCard, path: &str, bytes: &[u8]) -> Result<(), String> {
        sd.write(path, bytes)?;
        sd.mark_changed(path)
    }

    /// Puts a file of the SD card back as it was: its bytes, or no file.
    pub fn put(sd: &SdCard, path: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        match bytes {
            Some(bytes) => Store::write(sd, path, bytes),
            None => {
                sd.remove(path)?;
                sd.mark_changed(path)
            }
        }
    }

    /// Writes a picture from RGBA pixels. The picture keeps the size and the format that it has in the game.
    pub fn set_picture(&self, sd: &SdCard, kind: Kind, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
        let (wanted_width, wanted_height, _) = self.picture(None, kind)?;
        if (width, height) != (wanted_width, wanted_height) {
            return Err(format!("The picture must have {wanted_width} × {wanted_height} pixels. This one has {width} × {height}."));
        }
        match kind {
            Kind::Icon(_) => {
                let image = Clim::from_rgba(width, height, rgba)?;
                Store::write(sd, &kind.path(), &lz::compress_lz11(&image.to_bytes()))
            }
            Kind::Picture(_, _, name) | Kind::Intro(name) => {
                let image = Clim::parse(&Store::image_in(kind, self.game_file(kind)?)?)?.with_rgba(rgba)?;
                self.replace_in_bundle(sd, kind, name, image.to_bytes())
            }
            _ => Err("This is not a picture.".to_string()),
        }
    }

    /// Puts the bytes of a picture in the menu file of the SD card (the other changes of the file stay).
    fn replace_in_bundle(&self, sd: &SdCard, kind: Kind, name: &str, image: Vec<u8>) -> Result<(), String> {
        let current = self.current_file(Some(sd), kind)?;
        let mut menu = Store::bundle_in(kind, &current)?;
        let file = menu.darc.files.iter_mut().find(|f| f.path.rsplit('/').next() == Some(name)).ok_or_else(|| format!("The menu file has no picture {name}."))?;
        file.data = image;
        // A file that is again as in the game leaves the SD card.
        if menu == Store::bundle_in(kind, &self.game_file(kind)?)? {
            Store::put(sd, &kind.path(), None)
        } else {
            Store::write(sd, &kind.path(), &Store::file_with(kind, &current, &menu)?)
        }
    }

    /// Gives a PSS icon the picture of another icon, as the game has it now.
    pub fn copy_icon(&self, sd: &SdCard, from: usize, to: usize) -> Result<(), String> {
        let bytes = self.current_file(Some(sd), Kind::Icon(from))?;
        Store::write(sd, &Kind::Icon(to).path(), &bytes)
    }

    /// Gives back the thing of the game.
    pub fn reset(&self, sd: &SdCard, kind: Kind) -> Result<(), String> {
        match kind {
            Kind::Picture(_, _, name) | Kind::Intro(name) => {
                let image = Store::image_in(kind, self.game_file(kind)?)?;
                self.replace_in_bundle(sd, kind, name, image)
            }
            _ => Store::put(sd, &kind.path(), None),
        }
    }

    /// The models of the game that can take the place of a model of this kind: their number and their name. A model of the
    /// overworld and the rider of the Soar sequence take the characters of the overworld; a model of the battles takes the models of
    /// the battles.
    pub fn models_for(&self, kind: Kind) -> Vec<(u16, String)> {
        match kind {
            Kind::Overworld(_) | Kind::Soaring(_) => self.characters.labels().into_iter().enumerate().map(|(number, name)| (number as u16, name)).collect(),
            Kind::Battle(_) => BATTLE_MODELS.iter().map(|&(number, name)| (u16::from(number), name.to_string())).collect(),
            _ => Vec::new(),
        }
    }

    /// The model of the list of [`Store::models_for`] as a thing of the game.
    fn model_of(kind: Kind, number: u16) -> Kind {
        match kind {
            Kind::Battle(_) => Kind::Battle(number as u8),
            _ => Kind::Overworld(number as usize),
        }
    }

    /// How many joints another model has, and how many of them the model of the game of `kind` has too: the motions move the joints
    /// by name, so only these move.
    pub fn shared_joints(&self, kind: Kind, number: u16) -> Result<(usize, usize), String> {
        let other = parse_model(&self.model(None, Store::model_of(kind, number))?)?;
        let own = parse_model(&self.model(None, kind)?)?;
        let shared = other.bones.iter().filter(|bone| own.bones.iter().any(|b| b.name == bone.name)).count();
        Ok((shared, other.bones.len()))
    }

    /// Gives a model of the player the 3D model of another model of the game (its meshes and its pictures). The motions of the
    /// player stay.
    pub fn use_model(&self, sd: &SdCard, kind: Kind, number: u16) -> Result<(), String> {
        let mut made = self.model(None, Store::model_of(kind, number))?;
        let original = self.model(None, kind)?;
        if parse_model(&made)?.bones.is_empty() {
            return Err("This model has no joints: it cannot take the place of a character that moves.".to_string());
        }
        if made.len() > original.len() * 2 {
            return Err(format!("This model is too big for the game here ({} KB, the old one has {} KB).", made.len() / 1024, original.len() / 1024));
        }
        match kind {
            Kind::Battle(_) if made == original => Store::put(sd, &kind.path(), None),
            Kind::Battle(_) => Store::write(sd, &kind.path(), &lz::compress_lz11(&made)),
            Kind::Overworld(_) | Kind::Soaring(_) => {
                let current = plain(self.current_file(Some(sd), kind)?)?;
                let place = model_part(&bundle::split(&current)?.1)?;
                made.resize(made.len().next_multiple_of(ALIGNMENT), 0);
                let pack = bundle::replace_part_with(&current, place, &made, ALIGNMENT)?;
                if pack == plain(self.game_file(kind)?)? {
                    Store::put(sd, &kind.path(), None)
                } else {
                    Store::write(sd, &kind.path(), &lz::compress_lz11(&pack))
                }
            }
            _ => Err("This is not a model.".to_string()),
        }
    }

    /// The BCH file of a model, with its pictures.
    pub fn model(&self, sd: Option<&SdCard>, kind: Kind) -> Result<Vec<u8>, String> {
        let file = plain(self.current_file(sd, kind)?)?;
        match kind {
            Kind::Overworld(_) | Kind::Soaring(_) => {
                let parts = bundle::split(&file)?.1;
                Ok(parts[model_part(&parts)?].to_vec())
            }
            Kind::Battle(_) => Ok(file),
            _ => Err("This is not a model.".to_string()),
        }
    }

    /// The GLB file of a model. The overworld models have their motions in it.
    pub fn export_model(&self, sd: Option<&SdCard>, kind: Kind) -> Result<Vec<u8>, String> {
        match kind {
            Kind::Overworld(pack) => self.characters.export(sd, pack),
            _ => sango_species::glb_of_bch(&self.model(sd, kind)?),
        }
    }

    /// Replaces a model by the one of a glTF or GLB file. The skeleton and the motions of the game stay.
    pub fn import_model(&self, sd: &SdCard, kind: Kind, file: &Path, scale: f64) -> Result<(), String> {
        match kind {
            Kind::Overworld(pack) => self.characters.import(sd, pack, file, scale),
            Kind::Battle(number) => {
                self.trainers.import_model(sd, number, file, scale)?;
                sd.mark_changed(&kind.path())
            }
            Kind::Soaring(_) => self.import_soaring(sd, kind, file, scale),
            _ => Err("This is not a model.".to_string()),
        }
    }

    /// The Soar pack: the motions, then the model. Only the model changes.
    fn import_soaring(&self, sd: &SdCard, kind: Kind, file: &Path, scale: f64) -> Result<(), String> {
        let current = plain(self.current_file(Some(sd), kind)?)?;
        let parts = bundle::split(&current)?.1;
        let place = model_part(&parts)?;
        let original = parts[place];
        let source = sango_modelport::gltf::read(file)?;
        let bones = parse_model(original)?.bones;
        let joints: Vec<Joint> = bones
            .iter()
            .map(|b| Joint { name: b.name.clone(), parent: b.parent.and_then(|p| bones.get(p)).map(|p| p.name.clone()).unwrap_or_default(), scale: b.scale, rot: b.rotation.map(f64::from), trans: b.translation })
            .collect();
        let made = sango_modelport::trainer::trainer_model_within(&source, original, &joints, scale, original.len() / 2, true)?;
        if made.len() > original.len() * 2 {
            return Err(format!("The new model is too big for the game ({} KB, the old one has {} KB). Use a model with fewer polygons or smaller pictures.", made.len() / 1024, original.len() / 1024));
        }
        let made = sango_core::model::normalize_for_engine(&made)?.unwrap_or(made);
        // The pictures must be readable before the file goes on the card.
        parse_textures(&made)?;
        let pack = bundle::replace_part_with(&current, place, &made, ALIGNMENT)?;
        Store::write(sd, &kind.path(), &lz::compress_lz11(&pack))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn setup(name: &str) -> Option<(Store, SdCard, std::path::PathBuf)> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        let folder = std::env::temp_dir().join(format!("sango_player_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        Some((Store::open(&Dump::open(&root).unwrap()).unwrap(), sd, folder))
    }

    /// With the dump: every thing of both players is read, and nothing is changed on an empty card.
    #[test]
    fn every_picture_and_model_of_both_players_is_read() {
        let Some((store, sd, folder)) = setup("read") else { return };
        assert_eq!(store.icon_count(), 303);
        for player in 0..PLAYERS.len() {
            for entry in entries(player) {
                assert!(!store.changed(Some(&sd), entry.kind), "{}", entry.name);
                if entry.kind.is_model() {
                    let model = parse_model(&store.model(Some(&sd), entry.kind).unwrap()).unwrap();
                    assert!(!model.meshes.is_empty(), "{} of {}", entry.name, PLAYERS[player]);
                } else {
                    let (width, height, rgba) = store.picture(Some(&sd), entry.kind).unwrap();
                    assert!(rgba.chunks(4).any(|p| p[3] > 0), "{} of {} is empty", entry.name, PLAYERS[player]);
                    assert_eq!(rgba.len(), width * height * 4);
                }
            }
        }
        for file in 1..=store.icon_count() {
            let (width, height, _) = store.picture(None, Kind::Icon(file)).unwrap();
            assert_eq!((width, height), (ICON_SIDE, ICON_SIDE), "icon {file}");
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: an icon is copied, drawn again and reset; the game format stays.
    #[test]
    fn an_icon_is_changed_and_reset() {
        let Some((store, sd, folder)) = setup("icon") else { return };
        let (mine, other) = (Kind::Icon(38), Kind::Icon(39));
        store.copy_icon(&sd, 39, 38).unwrap();
        assert!(store.changed(Some(&sd), mine));
        assert_eq!(store.picture(Some(&sd), mine).unwrap(), store.picture(None, other).unwrap());
        assert!(sd.read("player/icon/index.bin").is_some(), "the list of the files is written");
        let (width, height, rgba) = store.picture(None, Kind::Icon(75)).unwrap();
        store.set_picture(&sd, mine, width, height, &rgba).unwrap();
        let written = lz::decompress(&sd.read(&mine.path()).unwrap()).unwrap();
        let game = plain(store.game_file(mine).unwrap()).unwrap();
        assert_eq!(written.len(), game.len(), "the file keeps the size of the game");
        let seen = store.picture(Some(&sd), mine).unwrap().2;
        assert!(seen.chunks(4).zip(rgba.chunks(4)).all(|(a, b)| (a[3] > 0) == (b[3] >= 128)));
        assert!(store.set_picture(&sd, mine, 20, 20, &[0; 1600]).is_err(), "a wrong size is refused");
        store.reset(&sd, mine).unwrap();
        assert!(!store.changed(Some(&sd), mine) && sd.read(&mine.path()).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: two pictures of one menu file change one by one, and the file leaves the card when both are back.
    #[test]
    fn the_pictures_of_a_menu_file_are_changed_one_by_one() {
        let Some((store, sd, folder)) = setup("menu") else { return };
        let (head, shadow) = (entries(0)[2].kind, entries(0)[3].kind);
        let (width, height, mut rgba) = store.picture(None, head).unwrap();
        for pixel in rgba.chunks_mut(4) {
            pixel.copy_from_slice(&[255, 0, 0, 255]);
        }
        store.set_picture(&sd, head, width, height, &rgba).unwrap();
        assert!(store.changed(Some(&sd), head) && !store.changed(Some(&sd), shadow));
        let seen = store.picture(Some(&sd), head).unwrap().2;
        assert!(seen[0] > 230 && seen[1] < 25 && seen[3] == 255, "{:?}", &seen[..4]);
        let (width, height, rgba) = store.picture(None, shadow).unwrap();
        store.set_picture(&sd, shadow, width, height, &rgba).unwrap();
        assert!(store.changed(Some(&sd), head), "the first change stays");
        store.reset(&sd, shadow).unwrap();
        store.reset(&sd, head).unwrap();
        assert!(sd.read(&head.path()).is_none(), "nothing else is changed: the file goes");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: the two portraits of the introduction are changed one by one, and the rest of the pack stays.
    #[test]
    fn the_portraits_of_the_introduction_are_changed() {
        let Some((store, sd, folder)) = setup("intro") else { return };
        let (brendan, may) = (entries(0)[1].kind, entries(1)[1].kind);
        let (width, height, original) = store.picture(None, brendan).unwrap();
        assert_eq!((width, height), (220, 220));
        // The portrait of May, put on Brendan.
        let (_, _, other) = store.picture(None, may).unwrap();
        store.set_picture(&sd, brendan, width, height, &other).unwrap();
        assert!(store.changed(Some(&sd), brendan) && !store.changed(Some(&sd), may));
        let seen = store.picture(Some(&sd), brendan).unwrap().2;
        // A pixel that nothing shows (no alpha) is written black.
        let same = seen.chunks(4).zip(other.chunks(4)).all(|(a, b)| a[3] == b[3] && (a[3] == 0 || a == b));
        assert!(same, "the format of the game keeps the colours");
        assert!(sd.read("player/intro/index.bin").is_some(), "the list of the files is written");
        let (written, game) = (sd.read(&brendan.path()).unwrap(), store.game_file(brendan).unwrap());
        let (a, b) = (bundle::split(&written).unwrap().1, bundle::split(&game).unwrap().1);
        assert_eq!(a.len(), b.len());
        assert!(a[1..] == b[1..], "the other parts of the pack stay");
        // The portrait of Brendan, on May: both portraits are changed in the same file.
        store.set_picture(&sd, may, width, height, &original).unwrap();
        assert!(store.changed(Some(&sd), brendan) && store.changed(Some(&sd), may));
        store.reset(&sd, brendan).unwrap();
        assert!(!store.changed(Some(&sd), brendan) && store.changed(Some(&sd), may));
        store.reset(&sd, may).unwrap();
        assert!(sd.read(&may.path()).is_none(), "nothing else is changed: the file goes");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: the models of the player take the model of another character; the motions stay, and the model of the player
    /// comes back.
    #[test]
    fn a_model_of_the_player_takes_another_model_of_the_game() {
        let Some((store, sd, folder)) = setup("other") else { return };
        // The overworld takes Steven or another character of the maps, the Soar rider the same, the battle takes Steven.
        let (overworld, soaring, battle) = (entries(0)[5].kind, entries(0)[10].kind, entries(0)[11].kind);
        assert_eq!((overworld, soaring, battle), (Kind::Overworld(172), Kind::Soaring(3), Kind::Battle(5)));
        for (kind, other) in [(overworld, 182), (soaring, 171), (battle, 8)] {
            assert!(store.models_for(kind).iter().any(|(number, _)| *number == other));
            let (shared, joints) = store.shared_joints(kind, other).unwrap();
            assert!(shared > 20 && shared <= joints, "{kind:?}: {shared} of {joints}");
            store.use_model(&sd, kind, other).unwrap();
            assert!(store.changed(Some(&sd), kind), "{kind:?}");
            let seen = parse_model(&store.model(Some(&sd), kind).unwrap()).unwrap();
            assert_eq!(seen.bones.len(), joints, "{kind:?}: the model of the other character");
            if let Kind::Overworld(_) | Kind::Soaring(_) = kind {
                let (new, old) = (lz::decompress(&sd.read(&kind.path()).unwrap()).unwrap(), plain(store.game_file(kind).unwrap()).unwrap());
                let (new, old) = (bundle::split(&new).unwrap().1, bundle::split(&old).unwrap().1);
                let place = model_part(&old).unwrap();
                assert!((0..old.len()).filter(|&n| n != place).all(|n| new[n] == old[n]), "{kind:?}: the motions stay");
            }
        }
        // The model of the player itself: the file leaves the card.
        store.use_model(&sd, battle, 5).unwrap();
        assert!(!store.changed(Some(&sd), battle));
        // Every model of the lists of the battles is readable.
        for (number, name) in store.models_for(battle) {
            assert!(store.shared_joints(battle, number).is_ok(), "{name}");
        }
        assert_eq!(store.shared_joints(overworld, 1).unwrap(), (0, 0));
        assert!(store.use_model(&sd, overworld, 1).is_err(), "a model without joints is refused");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: the Soar model and the battle model are exported, imported and read again.
    #[test]
    fn the_soaring_and_the_battle_models_are_exported_and_imported() {
        let Some((store, sd, folder)) = setup("model") else { return };
        for kind in [Kind::Soaring(3), Kind::Battle(5)] {
            let before = parse_model(&store.model(None, kind).unwrap()).unwrap();
            let file = folder.join("model.glb");
            std::fs::write(&file, store.export_model(None, kind).unwrap()).unwrap();
            store.import_model(&sd, kind, &file, 1.0).unwrap();
            assert!(store.changed(Some(&sd), kind), "{kind:?}");
            let after = parse_model(&store.model(Some(&sd), kind).unwrap()).unwrap();
            assert_eq!(before.bones.len(), after.bones.len(), "{kind:?}: the skeleton");
            assert_eq!(before.meshes.len(), after.meshes.len(), "{kind:?}: the meshes");
            store.reset(&sd, kind).unwrap();
            assert!(!store.changed(Some(&sd), kind));
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
