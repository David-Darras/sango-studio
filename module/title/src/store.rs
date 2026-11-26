//! The files of the title screen.
//!
//! | What | File of the game | File on the SD card |
//! | --- | --- | --- |
//! | Sequence of each screen | archive 162, files 3 and 86 (Alpha Sapphire), 2 and 85 (Omega Ruby) | `title/sequence/<file>.bin` (not compressed) |
//! | Pictures of the 3D scene | archive 152, the models that the sequences show (compressed) | `title/data/<file>.bin` |
//! | Pictures of the logo, one file for each language | archive 152, the layouts that the top sequence shows (compressed) | `layout/152/<file>.bin` |
//!
//! The Layout page can change the same layout files: a picture is put in the file of the SD card and the rest stays. A file that is
//! again as in the game leaves the SD card.

use crate::commands::{CONDITIONS, LAYOUT_SHOW, MODEL_SHOW};
use crate::sequence::Sequence;
use sango_core::{lz, model, Dump, Garc, SdCard};
use sango_layout::bundle::Bundle;
use std::path::Path;

const DATA_ARCHIVE: usize = 152;
const SEQUENCE_ARCHIVE: usize = 162;
/// The title screens of the archives: the one of the game first.
pub const VERSIONS: [&str; 2] = ["Alpha Sapphire", "Omega Ruby"];
/// For each title screen, the files of the sequences of the top screen and of the bottom screen.
pub const SEQUENCE_FILES: [[usize; 2]; 2] = [[3, 86], [2, 85]];
pub const SCREENS: [&str; 2] = ["Top screen", "Bottom screen"];

/// The pixels of a picture: its width, its height and its RGBA pixels from the top.
type Pixels = (usize, usize, Vec<u8>);

/// What a picture is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// A model of the 3D scene, on a screen.
    Model,
    /// The layout of the logo for one language (the number of the condition).
    Logo(u16),
}

/// A picture of the title screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// The title screen that shows it (a place of [`VERSIONS`]).
    pub version: usize,
    pub holder: Holder,
    pub file: usize,
    pub name: String,
    /// The text of the list.
    pub label: String,
}

impl Picture {
    /// The name of its PNG file when all the pictures go in a folder.
    pub fn file_name(&self) -> String {
        let version = VERSIONS[self.version].to_ascii_lowercase().replace(' ', "_");
        let name = self.name.trim_end_matches(".bclim").replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_");
        match self.holder {
            Holder::Logo(condition) => {
                let language = CONDITIONS.iter().find(|(number, _)| *number == condition).map_or("language", |(_, name)| name).to_ascii_lowercase();
                format!("{version}_{language}_{name}.png")
            }
            // Two models can have a picture of the same name: the number of the file tells them apart.
            Holder::Model => format!("{version}_scene{}_{name}.png", self.file),
        }
    }

    /// The place of its file on the SD card.
    pub fn path(&self) -> String {
        match self.holder {
            Holder::Model => format!("title/data/{}.bin", self.file),
            Holder::Logo(_) => format!("layout/{DATA_ARCHIVE}/{}.bin", self.file),
        }
    }
}

fn plain(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if lz::is_compressed(&bytes) {
        lz::decompress(&bytes)
    } else {
        Ok(bytes)
    }
}

/// The rows of a picture in the other order: a model keeps its pictures upside down.
fn flip_rows(rgba: &[u8], width: usize) -> Vec<u8> {
    rgba.chunks(width.max(1) * 4).rev().flatten().copied().collect()
}

fn sequence_path(version: usize, screen: usize) -> String {
    format!("title/sequence/{}.bin", SEQUENCE_FILES[version][screen])
}

/// Writes a file on the SD card, or removes it when `bytes` is `None`.
pub fn put(sd: &SdCard, path: &str, bytes: Option<&[u8]>) -> Result<(), String> {
    match bytes {
        Some(bytes) => sd.write(path, bytes).map(|_| ())?,
        None => {
            if sd.read(path).is_none() {
                return Ok(());
            }
            sd.remove(path).map(|_| ())?;
        }
    }
    sd.mark_changed(path)
}

pub struct Store {
    data: Garc,
    sequences: Garc,
    /// The sequences of the game: for each title screen, the one of each screen.
    pub game: [[Sequence; 2]; 2],
    /// The pictures of the two title screens: for each one, the logo of each language, then the 3D scene.
    pub pictures: Vec<Picture>,
}

impl Store {
    pub fn open(dump: &Dump) -> Result<Store, String> {
        let data = Garc::open(&dump.archive(DATA_ARCHIVE))?;
        let sequences = Garc::open(&dump.archive(SEQUENCE_ARCHIVE))?;
        let game_of = |version: usize, screen: usize| {
            let file = SEQUENCE_FILES[version][screen];
            let bytes = sequences.first_file(file).ok_or_else(|| format!("The sequence {file} is not in the archive."))?;
            Sequence::parse(bytes)
        };
        let game = [[game_of(0, 0)?, game_of(0, 1)?], [game_of(1, 0)?, game_of(1, 1)?]];
        let mut store = Store { data, sequences, game, pictures: Vec::new() };
        store.pictures = store.find_pictures()?;
        Ok(store)
    }

    /// The pictures of the files that the sequences of the game show.
    fn find_pictures(&self) -> Result<Vec<Picture>, String> {
        let mut found: Vec<Picture> = Vec::new();
        for (version, screens) in self.game.iter().enumerate() {
            // The logo, in the order of the languages.
            for (condition, _) in CONDITIONS.iter().skip(1) {
                for step in screens[0].steps.iter().filter(|step| step.kind == LAYOUT_SHOW && step.condition == *condition) {
                    let file = step.int(0) as usize;
                    let bundle = Bundle::parse(&self.game_file(file)?)?;
                    for inside in bundle.darc.files.iter().filter(|f| f.path.ends_with(".bclim")) {
                        let name = inside.path.rsplit('/').next().unwrap_or_default().to_string();
                        let label = name.trim_end_matches(".bclim").to_string();
                        found.push(Picture { version, holder: Holder::Logo(*condition), file, name, label });
                    }
                }
            }
            for (screen, sequence) in screens.iter().enumerate() {
                for step in sequence.steps.iter().filter(|step| step.kind == MODEL_SHOW) {
                    let file = step.int(0) as usize;
                    if found.iter().any(|p| p.version == version && p.holder == Holder::Model && p.file == file) {
                        continue;
                    }
                    for texture in model::parse_textures(&plain(self.game_file(file)?)?)? {
                        // Two models can have a picture of the same name.
                        let base = format!("{}: {}", SCREENS[screen], texture.name);
                        let same = found.iter().filter(|p| p.version == version && p.holder == Holder::Model && p.name == texture.name).count();
                        let label = if same == 0 { base } else { format!("{base} ({})", same + 1) };
                        found.push(Picture { version, holder: Holder::Model, file, name: texture.name, label });
                    }
                }
            }
        }
        Ok(found)
    }

    fn game_file(&self, file: usize) -> Result<Vec<u8>, String> {
        self.data.first_file(file).map(<[u8]>::to_vec).ok_or_else(|| format!("The file {file} is not in the archive."))
    }

    fn current_file(&self, sd: Option<&SdCard>, picture: &Picture) -> Result<Vec<u8>, String> {
        match sd.and_then(|sd| sd.read(&picture.path())) {
            Some(bytes) => Ok(bytes),
            None => self.game_file(picture.file),
        }
    }

    /// The BCLIM image of a logo picture in a layout file.
    fn logo_image(file: &[u8], name: &str) -> Result<Vec<u8>, String> {
        let bundle = Bundle::parse(file)?;
        bundle.darc.files.iter().find(|f| f.path.rsplit('/').next() == Some(name)).map(|f| f.data.clone()).ok_or_else(|| format!("The file has no picture {name}."))
    }

    fn read_picture(file: Vec<u8>, picture: &Picture) -> Result<(usize, usize, Vec<u8>), String> {
        match picture.holder {
            Holder::Logo(_) => {
                let image = sango_layout::tex::decode(&Store::logo_image(&file, &picture.name)?)?;
                Ok((image.width, image.height, image.rgba))
            }
            Holder::Model => {
                let found = model::parse_textures(&plain(file)?)?.into_iter().find(|t| t.name == picture.name);
                let texture = found.ok_or_else(|| format!("The model has no picture {}.", picture.name))?;
                Ok((texture.width, texture.height, flip_rows(&texture.rgba, texture.width)))
            }
        }
    }

    /// A picture: its width, its height and its RGBA pixels from the top.
    pub fn picture(&self, sd: Option<&SdCard>, picture: &Picture) -> Result<(usize, usize, Vec<u8>), String> {
        Store::read_picture(self.current_file(sd, picture)?, picture)
    }

    /// For each picture of [`Store::pictures`], true when the SD card changes it. Each file is read once.
    pub fn changed_pictures(&self, sd: Option<&SdCard>) -> Vec<bool> {
        let mut changed = vec![false; self.pictures.len()];
        let mut done: Vec<String> = Vec::new();
        for picture in &self.pictures {
            let path = picture.path();
            if done.contains(&path) {
                continue;
            }
            let card = sd.and_then(|sd| sd.read(&path));
            let game = self.game_file(picture.file);
            done.push(path.clone());
            let (Some(card), Ok(game)) = (card, game) else { continue };
            match picture.holder {
                Holder::Logo(_) => {
                    let images = |bytes: &[u8]| Bundle::parse(bytes).map(|bundle| bundle.darc.files).unwrap_or_default();
                    let (card, game) = (images(&card), images(&game));
                    for (place, other) in self.pictures.iter().enumerate().filter(|(_, p)| p.path() == path) {
                        let of = |files: &[sango_layout::darc::File]| files.iter().find(|f| f.path.rsplit('/').next() == Some(other.name.as_str())).map(|f| f.data.clone());
                        changed[place] = of(&card) != of(&game);
                    }
                }
                Holder::Model => {
                    let textures = |bytes: Vec<u8>| plain(bytes).and_then(|bytes| model::parse_textures(&bytes)).unwrap_or_default();
                    let (card, game) = (textures(card), textures(game));
                    for (place, other) in self.pictures.iter().enumerate().filter(|(_, p)| p.path() == path) {
                        changed[place] = card.iter().find(|t| t.name == other.name) != game.iter().find(|t| t.name == other.name);
                    }
                }
            }
        }
        changed
    }

    /// Puts the BCLIM image of a logo picture in the layout file of the SD card, or removes the file when it is again as in the game.
    fn write_logo(&self, sd: &SdCard, picture: &Picture, image: Vec<u8>) -> Result<(), String> {
        let mut bundle = Bundle::parse(&self.current_file(Some(sd), picture)?)?;
        let inside = bundle.darc.files.iter_mut().find(|f| f.path.rsplit('/').next() == Some(picture.name.as_str())).ok_or_else(|| format!("The file has no picture {}.", picture.name))?;
        inside.data = image;
        let same = bundle == Bundle::parse(&self.game_file(picture.file)?)?;
        put(sd, &picture.path(), (!same).then(|| bundle.to_bytes()).as_deref())
    }

    /// Writes a model file on the SD card, or removes it when it is again as in the game.
    fn write_model(&self, sd: &SdCard, picture: &Picture, made: Vec<u8>) -> Result<(), String> {
        let same = made == plain(self.game_file(picture.file)?)?;
        put(sd, &picture.path(), (!same).then(|| lz::compress_lz11(&made)).as_deref())
    }

    /// Changes a picture. It keeps the size and the format that it has in the game.
    pub fn set_picture(&self, sd: &SdCard, picture: &Picture, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
        let (wanted_width, wanted_height, _) = self.picture(None, picture)?;
        if (width, height) != (wanted_width, wanted_height) {
            return Err(format!("The picture must have {wanted_width} × {wanted_height} pixels. This one has {width} × {height}."));
        }
        match picture.holder {
            Holder::Logo(_) => {
                let original = Store::logo_image(&self.game_file(picture.file)?, &picture.name)?;
                self.write_logo(sd, picture, sango_layout::tex::encode(&original, rgba)?)
            }
            Holder::Model => {
                let current = plain(self.current_file(Some(sd), picture)?)?;
                self.write_model(sd, picture, model::replace_texture(&current, &picture.name, &flip_rows(rgba, width))?)
            }
        }
    }

    /// Gives back the picture of the game.
    pub fn reset_picture(&self, sd: &SdCard, picture: &Picture) -> Result<(), String> {
        if sd.read(&picture.path()).is_none() {
            return Ok(());
        }
        let game = self.game_file(picture.file)?;
        match picture.holder {
            Holder::Logo(_) => self.write_logo(sd, picture, Store::logo_image(&game, &picture.name)?),
            Holder::Model => {
                // The bytes of the game, not the pixels written again: a compressed picture does not keep them.
                let current = plain(self.current_file(Some(sd), picture)?)?;
                self.write_model(sd, picture, model::copy_texture(&current, &plain(game)?, &picture.name)?)
            }
        }
    }

    /// Gives back all the pictures of a title screen, one file after the other.
    pub fn reset_pictures(&self, sd: &SdCard, version: usize) -> Result<(), String> {
        let mut done: Vec<String> = Vec::new();
        for picture in self.pictures.iter().filter(|picture| picture.version == version) {
            let path = picture.path();
            if done.contains(&path) || sd.read(&path).is_none() {
                continue;
            }
            done.push(path.clone());
            match picture.holder {
                // Only the pictures change in the models of the studio.
                Holder::Model => put(sd, &path, None)?,
                // The Layout page can change the rest of a layout file: it stays.
                Holder::Logo(_) => {
                    let game = self.game_file(picture.file)?;
                    let original = Bundle::parse(&game)?;
                    let mut bundle = Bundle::parse(&self.current_file(Some(sd), picture)?)?;
                    for other in self.pictures.iter().filter(|p| p.path() == path) {
                        let name = |f: &sango_layout::darc::File| f.path.rsplit('/').next() == Some(other.name.as_str());
                        if let (Some(inside), Some(source)) = (bundle.darc.files.iter_mut().find(|f| name(f)), original.darc.files.iter().find(|f| name(f))) {
                            inside.data = source.data.clone();
                        }
                    }
                    put(sd, &path, (bundle != original).then(|| bundle.to_bytes()).as_deref())?;
                }
            }
        }
        Ok(())
    }

    /// The pictures of one file, decoded together: `places` are places of [`Store::pictures`] that this file holds.
    fn read_file(&self, file: Vec<u8>, places: &[usize]) -> Result<Vec<Pixels>, String> {
        let missing = |name: &str| format!("The file has no picture {name}.");
        match self.pictures[places[0]].holder {
            Holder::Logo(_) => {
                let bundle = Bundle::parse(&file)?;
                places
                    .iter()
                    .map(|&place| {
                        let name = &self.pictures[place].name;
                        let inside = bundle.darc.files.iter().find(|f| f.path.rsplit('/').next() == Some(name.as_str())).ok_or_else(|| missing(name))?;
                        let image = sango_layout::tex::decode(&inside.data)?;
                        Ok((image.width, image.height, image.rgba))
                    })
                    .collect()
            }
            Holder::Model => {
                let textures = model::parse_textures(&plain(file)?)?;
                places
                    .iter()
                    .map(|&place| {
                        let name = &self.pictures[place].name;
                        let texture = textures.iter().find(|t| &t.name == name).ok_or_else(|| missing(name))?;
                        Ok((texture.width, texture.height, flip_rows(&texture.rgba, texture.width)))
                    })
                    .collect()
            }
        }
    }

    /// The places of the pictures of a title screen, file by file.
    fn by_file(&self, version: usize) -> Vec<Vec<usize>> {
        let mut files: Vec<(String, Vec<usize>)> = Vec::new();
        for (place, picture) in self.pictures.iter().enumerate().filter(|(_, p)| p.version == version) {
            let path = picture.path();
            match files.iter_mut().find(|(of, _)| *of == path) {
                Some((_, places)) => places.push(place),
                None => files.push((path, vec![place])),
            }
        }
        files.into_iter().map(|(_, places)| places).collect()
    }

    /// Writes every picture of a title screen in a folder, one PNG file for each one (see [`Picture::file_name`]). Gives their
    /// number.
    pub fn export_pictures(&self, sd: Option<&SdCard>, version: usize, folder: &Path) -> Result<usize, String> {
        let mut count = 0;
        for places in self.by_file(version) {
            let pictures = self.read_file(self.current_file(sd, &self.pictures[places[0]])?, &places)?;
            for (&place, (width, height, rgba)) in places.iter().zip(pictures) {
                let path = folder.join(self.pictures[place].file_name());
                let image = image::RgbaImage::from_raw(width as u32, height as u32, rgba).ok_or("A picture is not complete.")?;
                image.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Reads the PNG files of a folder that have the names of [`Store::export_pictures`] and puts the new ones in their files. A
    /// picture that did not change is not written again (a compressed format would lose a little); one that is again the one of the
    /// game takes the bytes of the game. Nothing is written when a file of the folder is wrong. Gives the number of changed pictures.
    pub fn import_pictures(&self, sd: &SdCard, version: usize, folder: &Path) -> Result<usize, String> {
        let mut writes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        let (mut found, mut count) = (0, 0);
        for places in self.by_file(version) {
            let given: Vec<usize> = places.iter().copied().filter(|&place| folder.join(self.pictures[place].file_name()).is_file()).collect();
            let Some(&first) = given.first() else { continue };
            found += given.len();
            let picture = &self.pictures[first];
            let game_file = self.game_file(picture.file)?;
            let current_file = self.current_file(Some(sd), picture)?;
            let now = self.read_file(current_file.clone(), &given)?;
            let game = self.read_file(game_file.clone(), &given)?;
            // The new pictures of this file: the place, the pixels, and whether they are the ones of the game.
            let mut new: Vec<(usize, Vec<u8>, bool)> = Vec::new();
            for ((&place, now), game) in given.iter().zip(now).zip(game) {
                let path = folder.join(self.pictures[place].file_name());
                let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
                if (image.width() as usize, image.height() as usize) != (now.0, now.1) {
                    return Err(format!("{}: the picture must have {} × {} pixels.", path.display(), now.0, now.1));
                }
                let rgba = image.into_raw();
                if rgba != now.2 {
                    new.push((place, rgba.clone(), rgba == game.2));
                }
            }
            if new.is_empty() {
                continue;
            }
            count += new.len();
            let bytes = match picture.holder {
                Holder::Logo(_) => {
                    let original = Bundle::parse(&game_file)?;
                    let mut bundle = Bundle::parse(&current_file)?;
                    for (place, rgba, is_game) in &new {
                        let name = &self.pictures[*place].name;
                        let of = |f: &sango_layout::darc::File| f.path.rsplit('/').next() == Some(name.as_str());
                        let source = original.darc.files.iter().find(|f| of(f)).ok_or("A picture is not in the file of the game.")?;
                        let data = if *is_game { source.data.clone() } else { sango_layout::tex::encode(&source.data, rgba)? };
                        bundle.darc.files.iter_mut().find(|f| of(f)).ok_or("A picture is not in the file.")?.data = data;
                    }
                    (bundle != original).then(|| bundle.to_bytes())
                }
                Holder::Model => {
                    let original = plain(game_file)?;
                    let mut made = plain(current_file)?;
                    for (place, rgba, is_game) in &new {
                        let name = &self.pictures[*place].name;
                        made = if *is_game {
                            model::copy_texture(&made, &original, name)?
                        } else {
                            let width = self.read_file(original.clone(), &[*place])?[0].0;
                            model::replace_texture(&made, name, &flip_rows(rgba, width))?
                        };
                    }
                    (made != original).then(|| lz::compress_lz11(&made))
                }
            };
            writes.push((picture.path(), bytes));
        }
        if found == 0 {
            return Err("The folder has no picture of this title screen. The files must have the names that Export all the pictures gives them.".to_string());
        }
        for (path, bytes) in writes {
            put(sd, &path, bytes.as_deref())?;
        }
        Ok(count)
    }

    /// The sequence of a screen of a title screen: the one of the SD card, else the one of the game.
    pub fn sequence(&self, sd: Option<&SdCard>, version: usize, screen: usize) -> Result<Sequence, String> {
        match sd.and_then(|sd| sd.read(&sequence_path(version, screen))) {
            Some(bytes) => Sequence::parse(&bytes),
            None => Ok(self.game[version][screen].clone()),
        }
    }

    pub fn sequence_changed(&self, sd: Option<&SdCard>, version: usize, screen: usize) -> bool {
        sd.is_some_and(|sd| sd.read(&sequence_path(version, screen)).is_some())
    }

    pub fn save_sequence(&self, sd: &SdCard, version: usize, screen: usize, sequence: &Sequence) -> Result<(), String> {
        let bytes = sequence.to_bytes();
        let game = self.sequences.first_file(SEQUENCE_FILES[version][screen]);
        put(sd, &sequence_path(version, screen), (Some(bytes.as_slice()) != game).then_some(bytes.as_slice()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pokemon;

    fn setup(name: &str) -> Option<(Store, SdCard, std::path::PathBuf)> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        let folder = std::env::temp_dir().join(format!("sango_title_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        Some((Store::open(&Dump::open(&root).unwrap()).unwrap(), sd, folder))
    }

    /// With the dump: the two sequences are read and written with the same bytes, in the order of their frames, and every picture
    /// is read.
    #[test]
    fn the_title_screen_of_the_game_is_read() {
        let Some((store, sd, folder)) = setup("read") else { return };
        for (version, name) in VERSIONS.iter().enumerate() {
            for screen in 0..2 {
                let mut sequence = store.sequence(None, version, screen).unwrap();
                assert_eq!(Some(sequence.to_bytes().as_slice()), store.sequences.first_file(SEQUENCE_FILES[version][screen]));
                sequence.settle(None);
                assert_eq!(sequence, store.game[version][screen], "{name}, screen {screen}: the steps are in the order of their frames");
            }
            let pictures: Vec<&Picture> = store.pictures.iter().filter(|p| p.version == version).collect();
            for (condition, language) in CONDITIONS.iter().skip(1) {
                assert!(pictures.iter().any(|p| p.holder == Holder::Logo(*condition)), "{name}: no logo for {language}");
            }
            assert!(pictures.iter().any(|p| p.holder == Holder::Model));
        }
        for picture in &store.pictures {
            let (width, height, rgba) = store.picture(None, picture).unwrap_or_else(|e| panic!("{} in {}: {e}", picture.name, picture.file));
            assert_eq!(rgba.len(), width * height * 4);
        }
        assert!(!store.changed_pictures(Some(&sd)).contains(&true));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: each picture takes new pixels (black) and comes back as in the game, and the file leaves the card.
    #[test]
    fn every_picture_is_replaced_and_reset() {
        let Some((store, sd, folder)) = setup("pictures") else { return };
        let mut seen = Vec::new();
        for picture in &store.pictures {
            // Two pictures of each file are enough.
            if seen.iter().filter(|&&file| file == picture.file).count() == 2 {
                continue;
            }
            seen.push(picture.file);
            let (width, height, _) = store.picture(None, picture).unwrap();
            let black: Vec<u8> = [0, 0, 0, 255].repeat(width * height);
            store.set_picture(&sd, picture, width, height, &black).unwrap_or_else(|e| panic!("{} in {}: {e}", picture.name, picture.file));
            assert!(store.set_picture(&sd, picture, width + 1, height, &black).is_err());
        }
        let changed = store.changed_pictures(Some(&sd));
        assert!(changed.contains(&true) && changed.contains(&false));
        // One picture of a model and one of a logo come back alone, then all the others together.
        for holder in [true, false] {
            let picture = store.pictures.iter().find(|p| (p.holder == Holder::Model) == holder).unwrap();
            store.reset_picture(&sd, picture).unwrap();
            let place = store.pictures.iter().position(|p| p == picture).unwrap();
            assert!(!store.changed_pictures(Some(&sd))[place], "{}", picture.label);
        }
        for version in 0..VERSIONS.len() {
            store.reset_pictures(&sd, version).unwrap();
        }
        for picture in &store.pictures {
            assert!(sd.read(&picture.path()).is_none(), "{}", picture.path());
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: all the pictures go in a folder and come back: nothing changes; two new files change two pictures only; the
    /// first files give the game back.
    #[test]
    fn all_the_pictures_go_in_a_folder_and_come_back() {
        let Some((store, sd, folder)) = setup("folder") else { return };
        for version in 0..VERSIONS.len() {
            let out = folder.join(format!("png{version}"));
            std::fs::create_dir_all(&out).unwrap();
            let pictures: Vec<usize> = (0..store.pictures.len()).filter(|&p| store.pictures[p].version == version).collect();
            assert_eq!(store.export_pictures(Some(&sd), version, &out), Ok(pictures.len()));
            assert_eq!(std::fs::read_dir(&out).unwrap().count(), pictures.len(), "each picture has its own name");
            assert_eq!(store.import_pictures(&sd, version, &out), Ok(0), "the same files change nothing");
            assert!(pictures.iter().all(|&p| sd.read(&store.pictures[p].path()).is_none()));
            // One picture of a logo and one of a model turn black.
            let chosen = [true, false].map(|model| pictures.iter().copied().find(|&p| (store.pictures[p].holder == Holder::Model) == model).unwrap());
            let originals = chosen.map(|p| std::fs::read(out.join(store.pictures[p].file_name())).unwrap());
            for &place in &chosen {
                let (width, height, _) = store.picture(None, &store.pictures[place]).unwrap();
                image::RgbaImage::from_pixel(width as u32, height as u32, image::Rgba([0, 0, 0, 255])).save(out.join(store.pictures[place].file_name())).unwrap();
            }
            assert_eq!(store.import_pictures(&sd, version, &out), Ok(2));
            let changed: Vec<usize> = store.changed_pictures(Some(&sd)).iter().enumerate().filter(|(_, &c)| c).map(|(p, _)| p).collect();
            let mut wanted = chosen.to_vec();
            wanted.sort_unstable();
            assert_eq!(changed, wanted);
            // A picture of a wrong size: nothing is written.
            let wrong = out.join(store.pictures[chosen[0]].file_name());
            image::RgbaImage::new(3, 3).save(&wrong).unwrap();
            assert!(store.import_pictures(&sd, version, &out).is_err());
            assert_eq!(store.changed_pictures(Some(&sd)).iter().filter(|&&c| c).count(), 2);
            for (&place, original) in chosen.iter().zip(&originals) {
                std::fs::write(out.join(store.pictures[place].file_name()), original).unwrap();
            }
            assert_eq!(store.import_pictures(&sd, version, &out), Ok(2));
            assert!(pictures.iter().all(|&p| sd.read(&store.pictures[p].path()).is_none()), "the files of the game leave the card");
        }
        let empty = folder.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(store.import_pictures(&sd, 0, &empty).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: another Pokémon takes the place of the ones of the game, with a size and a place; the Pokémon of the game
    /// gives the bytes of the game back.
    #[test]
    fn the_pokemon_changes_and_comes_back() {
        let Some((store, sd, folder)) = setup("pokemon") else { return };
        for (version, species) in [(0, 382), (1, 383)] {
        let game = &store.game[version][1];
        let groups = pokemon::groups(game);
        assert_eq!(groups.len(), 2);
        let first: Vec<(usize, usize, i32)> = groups.iter().map(|&g| pokemon::pokemon(game, g).unwrap()).collect();
        assert_eq!(first.iter().map(|p| (p.0, p.1)).collect::<Vec<_>>(), [(species, 1), (species, 0)]);
        let mut sequence = game.clone();
        for &group in &groups {
            pokemon::set_pokemon(&mut sequence, game, group, 25, 0, 0);
            pokemon::set_size(&mut sequence, game, group, 2.0);
            pokemon::set_place(&mut sequence, game, group, [0.0, -10.0, 0.0]);
        }
        store.save_sequence(&sd, version, 1, &sequence).unwrap();
        let again = store.sequence(Some(&sd), version, 1).unwrap();
        assert_eq!(again, sequence);
        for &group in &groups {
            assert_eq!(pokemon::pokemon(&again, group).unwrap().0, 25);
            assert_eq!((pokemon::size(&again, group), pokemon::place(&again, group)[1]), (2.0, -10.0));
        }
        assert!(!again.steps.iter().any(|s| s.kind == crate::commands::POKEMON_MOTION_FILES));
        for (&group, original) in groups.iter().zip(&first) {
            pokemon::set_pokemon(&mut sequence, game, group, original.0, original.1, original.2);
            pokemon::set_size(&mut sequence, game, group, 1.0);
            pokemon::set_place(&mut sequence, game, group, [0.0; 3]);
        }
        let key = |s: &crate::sequence::Step| (s.start, s.group, s.kind, s.data.clone(), s.end);
        let (mut now, mut then): (Vec<_>, Vec<_>) = (sequence.steps.iter().map(key).collect(), game.steps.iter().map(key).collect());
        now.sort();
        then.sort();
        assert_eq!(now, then);
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
