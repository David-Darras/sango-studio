//! The files of the data of the maps.
//!
//! | What | File of the game | File on the SD card |
//! | --- | --- | --- |
//! | Settings of a map (56 bytes) and its water (12 bytes) | archive 13, file of the map: parts 0 and 4 of its pack | `overworld/zone/<map>.bin` |
//! | Settings of all the maps in one table (the game reads it when it starts) | archive 13, file 536 | `overworld/zone/536.bin` |
//! | Camera that an area uses (2 bytes at 10 in the 44 bytes of the area) | archive 14, file 228 | `overworld/area/228.bin` |
//! | The 16 cameras of an area (68 bytes each) | archive 14, file of the area: part 6 of its pack | `overworld/area/<area>.bin` |
//! | The names of the places | the texts of each language, file 90 | `game_text/<language>/90.bin` |
//!
//! A map pack has also the events (part 1, the page 3D Maps), the scripts (part 2) and the wild Pokémon (part 3, the page
//! Encounter): they stay as they are. A file that is again the one of the game leaves the SD card.

use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{bundle, file_lists, lz, Dump, Garc, SdCard};
use std::collections::BTreeMap;
use std::path::PathBuf;

const ZONE_ARCHIVE: usize = 13;
const AREA_ARCHIVE: usize = 14;
/// The file of the texts that has the names of the places: a name, then its second text, for each place.
pub const NAME_FILE: usize = 90;

pub const SETTINGS_SIZE: usize = 56;
pub const WATER_SIZE: usize = 12;
const SETTINGS_PART: usize = 0;
const WATER_PART: usize = 4;
const CAMERA_PART: usize = 6;
/// The number of cameras of an area, and the bytes of one.
pub const CAMERA_COUNT: usize = 16;
pub const CAMERA_SIZE: usize = 68;
const AREA_ENTRY: usize = 44;
const AREA_CAMERA_AT: usize = 10;

/// One value in the settings of a map: its name in the CSV file, its bytes and its bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub key: &'static str,
    /// The first byte, and the number of bytes (1, 2 or 4) that hold the value.
    at: usize,
    bytes: usize,
    shift: u32,
    bits: u32,
    signed: bool,
}

const fn field(key: &'static str, at: usize, bytes: usize, shift: u32, bits: u32) -> Field {
    Field { key, at, bytes, shift, bits, signed: false }
}

const fn position(key: &'static str, at: usize) -> Field {
    Field { key, at, bytes: 2, shift: 0, bits: 16, signed: true }
}

impl Field {
    /// The least and the most value that the field can have.
    pub fn range(self) -> (i64, i64) {
        if self.signed {
            (-(1 << (self.bits - 1)), (1 << (self.bits - 1)) - 1)
        } else {
            (0, (1i64 << self.bits) - 1)
        }
    }
}

pub const RESOURCE_KIND: Field = field("resource_kind", 0, 1, 0, 8);
pub const MODEL_SET: Field = field("character_model_set", 1, 1, 0, 8);
pub const AREA: Field = field("area", 2, 2, 0, 16);
pub const MATRIX: Field = field("matrix", 4, 2, 0, 16);
pub const SCRIPT_TEXT: Field = field("script_text_file", 6, 2, 0, 16);
pub const MUSIC: Field = field("music", 8, 4, 0, 32);
pub const EVENT_FILE: Field = field("event_file", 24, 2, 0, 16);
pub const GROUP: Field = field("town_map_group", 26, 2, 0, 16);
pub const PLACE_NAME: Field = field("place_name", 28, 2, 0, 10);
pub const NAME_WINDOW: Field = field("name_window", 28, 2, 10, 6);
pub const WEATHER: Field = field("weather", 30, 2, 0, 5);
pub const SKY: Field = field("sky", 30, 2, 5, 1);
pub const ROLLER_SKATES: Field = field("roller_skates", 30, 2, 6, 1);
pub const BATTLE_BACKGROUND: Field = field("battle_background", 30, 2, 7, 7);
pub const MAP_CHANGE: Field = field("map_change", 32, 2, 0, 5);
pub const BICYCLE: Field = field("bicycle", 32, 2, 10, 1);
pub const RUNNING: Field = field("running", 32, 2, 11, 1);
pub const ESCAPE_ROPE: Field = field("escape_rope", 32, 2, 12, 1);
pub const FLY: Field = field("fly", 32, 2, 13, 1);
pub const BICYCLE_MUSIC: Field = field("bicycle_music", 32, 2, 14, 1);
pub const UNIQUE_SEQUENCE: Field = field("special_sequence", 38, 2, 0, 16);
pub const BLOOM: Field = field("bloom", 40, 4, 0, 1);
pub const FLASH: Field = field("flash", 40, 4, 1, 1);
pub const REVERB: Field = field("reverb", 40, 4, 3, 1);
pub const MEMORY: Field = field("pokemon_memory", 40, 4, 4, 7);
pub const IDLE_MOTION: Field = field("idle_motion", 40, 4, 11, 1);
pub const BREATH: Field = field("visible_breath", 40, 4, 12, 1);
pub const DOWSING: Field = field("dowsing", 40, 4, 13, 1);
pub const COMMUNICATION: Field = field("communication", 40, 4, 14, 1);
pub const STEREO: Field = field("stereo_3d", 40, 4, 16, 1);
pub const LIGHTS: Field = field("lights", 40, 4, 17, 4);
pub const FLY_MAP: Field = field("fly_map", 40, 4, 21, 10);
pub const FLY_X: Field = position("fly_x", 44);
pub const FLY_Y: Field = position("fly_y", 46);
pub const FLY_Z: Field = position("fly_z", 48);
pub const START_X: Field = position("start_x", 50);
pub const START_Y: Field = position("start_y", 52);
pub const START_Z: Field = position("start_z", 54);

/// Every field of the settings, in the order of the CSV file.
pub const FIELDS: [Field; 38] = [
    PLACE_NAME, NAME_WINDOW, MUSIC, WEATHER, BATTLE_BACKGROUND, MAP_CHANGE, FLY, BICYCLE, BICYCLE_MUSIC, RUNNING, ROLLER_SKATES, ESCAPE_ROPE,
    DOWSING, COMMUNICATION, SKY, BLOOM, FLASH, REVERB, STEREO, IDLE_MOTION, BREATH, LIGHTS, FLY_MAP, FLY_X, FLY_Y, FLY_Z, START_X, START_Y,
    START_Z, AREA, MATRIX, SCRIPT_TEXT, EVENT_FILE, GROUP, UNIQUE_SEQUENCE, MEMORY, RESOURCE_KIND, MODEL_SET,
];

/// The settings of a map, as the game has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings(pub [u8; SETTINGS_SIZE]);

impl Settings {
    pub fn parse(bytes: &[u8]) -> Result<Settings, String> {
        bytes.get(..SETTINGS_SIZE).map(|b| Settings(b.try_into().unwrap())).ok_or_else(|| "The settings of the map are too short.".to_string())
    }

    fn word(&self, field: Field) -> u64 {
        (0..field.bytes).fold(0u64, |value, i| value | u64::from(self.0[field.at + i]) << (8 * i))
    }

    pub fn get(&self, field: Field) -> i64 {
        let value = (self.word(field) >> field.shift) & ((1u64 << field.bits) - 1);
        if field.signed && value >> (field.bits - 1) != 0 {
            value as i64 - (1i64 << field.bits)
        } else {
            value as i64
        }
    }

    /// Changes a value; it stays in the range of the field.
    pub fn set(&mut self, field: Field, value: i64) {
        let (low, high) = field.range();
        let mask = (1u64 << field.bits) - 1;
        let bits = value.clamp(low, high) as u64 & mask;
        let word = (self.word(field) & !(mask << field.shift)) | bits << field.shift;
        for i in 0..field.bytes {
            self.0[field.at + i] = (word >> (8 * i)) as u8;
        }
    }
}

/// The water of a map: four surfaces, each one on or off, with its height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Water(pub [u8; WATER_SIZE]);

impl Water {
    pub fn parse(bytes: &[u8]) -> Result<Water, String> {
        bytes.get(..WATER_SIZE).map(|b| Water(b.try_into().unwrap())).ok_or_else(|| "The water of the map is too short.".to_string())
    }

    pub fn on(&self, surface: usize) -> bool {
        self.0[surface] != 0
    }

    pub fn set_on(&mut self, surface: usize, on: bool) {
        self.0[surface] = u8::from(on);
    }

    pub fn height(&self, surface: usize) -> i16 {
        i16::from_le_bytes([self.0[4 + 2 * surface], self.0[5 + 2 * surface]])
    }

    pub fn set_height(&mut self, surface: usize, height: i16) {
        self.0[4 + 2 * surface..6 + 2 * surface].copy_from_slice(&height.to_le_bytes());
    }
}

/// A value of a camera: its name, its place and what it does.
pub struct CameraValue {
    pub name: &'static str,
    pub note: &'static str,
    pub at: usize,
    pub speed: f64,
}

/// The decimal values of a camera that the page shows.
pub const CAMERA_VALUES: [CameraValue; 11] = [
    CameraValue { name: "Distance", note: "How far the camera is from the point that it looks at.", at: 36, speed: 1.0 },
    CameraValue { name: "Angle up and down", note: "In degrees. A value under 0 looks down.", at: 12, speed: 0.25 },
    CameraValue { name: "Angle left and right", note: "In degrees.", at: 16, speed: 0.25 },
    CameraValue { name: "Roll", note: "In degrees: the picture leans to a side.", at: 20, speed: 0.25 },
    CameraValue { name: "Field of view", note: "In degrees. A large value shows more and makes things smaller.", at: 32, speed: 0.25 },
    CameraValue { name: "Shift left and right", note: "The point that the camera looks at, from the player.", at: 4, speed: 0.25 },
    CameraValue { name: "Shift up and down", note: "The point that the camera looks at, from the player.", at: 8, speed: 0.25 },
    CameraValue { name: "Near limit", note: "What is nearer than this does not show.", at: 24, speed: 1.0 },
    CameraValue { name: "Far limit", note: "What is farther than this does not show.", at: 28, speed: 10.0 },
    CameraValue { name: "3D depth range", note: "A value of the 3D effect of the camera.", at: 40, speed: 0.1 },
    CameraValue { name: "3D depth level", note: "A value of the 3D effect of the camera.", at: 44, speed: 0.1 },
];

/// One camera of an area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera(pub [u8; CAMERA_SIZE]);

impl Camera {
    pub fn value(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    pub fn set_value(&mut self, at: usize, value: f32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// The number of frames that the camera takes to move to this view.
    pub fn blend_frames(&self) -> u32 {
        u32::from_le_bytes(self.0[56..60].try_into().unwrap())
    }

    pub fn set_blend_frames(&mut self, frames: u32) {
        self.0[56..60].copy_from_slice(&frames.to_le_bytes());
    }
}

/// Everything that the page edits for a map.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub zone: usize,
    pub settings: Settings,
    pub water: Water,
    /// The area that the cameras are of.
    pub area: usize,
    /// The camera that the area uses.
    pub camera: u16,
    pub cameras: Vec<Camera>,
    /// The name of the place in each language: the language, the line, the text.
    pub names: BTreeMap<(usize, usize), String>,
}

fn plain(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if lz::is_compressed(&bytes) {
        lz::decompress(&bytes)
    } else {
        Ok(bytes)
    }
}

/// The text of a line in the file of a language.
pub fn name_of(files: &[Option<TextFile>], language: usize, line: usize) -> Option<String> {
    files.get(language)?.as_ref()?.text(line)
}

/// Reads the name of the place of the map in every language, when the edit does not have it yet.
pub fn add_names(files: &[Option<TextFile>], edit: &mut Edit) {
    let line = edit.settings.get(PLACE_NAME) as usize;
    for language in 0..files.len() {
        if !edit.names.contains_key(&(language, line)) {
            if let Some(name) = name_of(files, language, line) {
                edit.names.insert((language, line), name);
            }
        }
    }
}

/// The archives of the dump.
pub struct Store {
    zones: Garc,
    areas: Garc,
    texts: Vec<Option<Garc>>,
    /// The largest compressed area file of the game: the room that the game has for one.
    area_room: usize,
}

/// The place of a file of the maps on the SD card.
fn card_file(sd: &SdCard, folder: &str, number: usize) -> PathBuf {
    sd.overworld_folder().join(folder).join(format!("{number}.bin"))
}

/// Writes a file of the maps and its list, or removes it when `bytes` is `None`.
fn put_map_file(sd: &SdCard, folder: &str, number: usize, bytes: Option<&[u8]>) -> Result<(), String> {
    let path = card_file(sd, folder, number);
    let list = sd.overworld_folder().join(folder).join("index.bin");
    let mut entries: BTreeMap<u32, u32> = std::fs::read(&list).ok().map(|old| file_lists::parse_index(&old)).transpose()?.unwrap_or_default().into_iter().collect();
    let failed = |e: std::io::Error| format!("{}: {e}", path.display());
    match bytes {
        Some(bytes) => {
            std::fs::create_dir_all(sd.overworld_folder().join(folder)).map_err(failed)?;
            std::fs::write(&path, bytes).map_err(failed)?;
            entries.insert(number as u32, bytes.len() as u32);
        }
        None => {
            if !path.exists() {
                return Ok(());
            }
            std::fs::remove_file(&path).map_err(failed)?;
            entries.remove(&(number as u32));
        }
    }
    if entries.is_empty() {
        let _ = std::fs::remove_file(&list);
    } else {
        let files: Vec<(u32, u32)> = entries.into_iter().collect();
        std::fs::write(&list, file_lists::index_bytes(&files)).map_err(|e| format!("{}: {e}", list.display()))?;
    }
    sd.mark_changed(&format!("overworld/{folder}/{number}.bin"))
}

impl Store {
    pub fn open(dump: &Dump) -> Result<Store, String> {
        let areas = Garc::open(&dump.archive(AREA_ARCHIVE))?;
        let area_room = (0..areas.file_count().saturating_sub(1)).filter_map(|n| areas.first_file(n).map(<[u8]>::len)).max().unwrap_or(0);
        let texts = LANGUAGES.iter().map(|language| Garc::open(&dump.archive(language.archive)).ok()).collect();
        Ok(Store { zones: Garc::open(&dump.archive(ZONE_ARCHIVE))?, areas, texts, area_room })
    }

    /// The number of maps: the files of the archive without the table of all the maps and the file after it.
    pub fn zone_count(&self) -> usize {
        self.zones.file_count().saturating_sub(2)
    }

    /// The file of the table of the settings of all the maps.
    fn table_file(&self) -> usize {
        self.zone_count()
    }

    /// The number of areas, and the file of the table of the areas.
    pub fn area_count(&self) -> usize {
        self.areas.file_count().saturating_sub(1)
    }

    /// A file of the maps as the game uses it now (the SD card first), as it is stored.
    fn raw(garc: &Garc, sd: Option<&SdCard>, folder: &str, number: usize) -> Result<Vec<u8>, String> {
        match sd.and_then(|sd| std::fs::read(card_file(sd, folder, number)).ok()) {
            Some(bytes) => Ok(bytes),
            None => garc.first_file(number).map(<[u8]>::to_vec).ok_or_else(|| format!("The file {number} is not in the archive.")),
        }
    }

    fn zone_pack(&self, sd: Option<&SdCard>, zone: usize) -> Result<Vec<u8>, String> {
        if zone >= self.zone_count() {
            return Err(format!("The game has no map {zone}."));
        }
        plain(Store::raw(&self.zones, sd, "zone", zone)?)
    }

    /// The settings and the water of a map.
    pub fn settings(&self, sd: Option<&SdCard>, zone: usize) -> Result<(Settings, Water), String> {
        let pack = self.zone_pack(sd, zone)?;
        let parts = bundle::split(&pack)?.1;
        let part = |n: usize| parts.get(n).copied().ok_or_else(|| "The file of the map has not all its parts.".to_string());
        Ok((Settings::parse(part(SETTINGS_PART)?)?, Water::parse(part(WATER_PART)?)?))
    }

    /// The camera that an area uses, and its cameras.
    pub fn cameras(&self, sd: Option<&SdCard>, area: usize) -> Result<(u16, Vec<Camera>), String> {
        if area >= self.area_count() {
            return Err(format!("The game has no area {area}."));
        }
        let table = plain(Store::raw(&self.areas, sd, "area", self.area_count())?)?;
        let at = area * AREA_ENTRY + AREA_CAMERA_AT;
        let used = table.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("The table of the areas is too short.")?;
        let pack = plain(Store::raw(&self.areas, sd, "area", area)?)?;
        let parts = bundle::split(&pack)?.1;
        let part = parts.get(CAMERA_PART).ok_or("The area has no camera.")?;
        let cameras = part.chunks_exact(CAMERA_SIZE).take(CAMERA_COUNT).map(|c| Camera(c.try_into().unwrap())).collect::<Vec<_>>();
        if cameras.len() != CAMERA_COUNT {
            return Err("The list of the cameras of the area is too short.".into());
        }
        Ok((used, cameras))
    }

    /// A file of the names of the places: the SD card first, then the game.
    pub fn name_file(&self, sd: Option<&SdCard>, language: usize) -> Option<TextFile> {
        let on_card = sd.and_then(|sd| sd.read(&text::sd_path(language, NAME_FILE))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        on_card.or_else(|| self.game_name_file(language))
    }

    pub fn game_name_file(&self, language: usize) -> Option<TextFile> {
        self.texts.get(language)?.as_ref()?.first_file(NAME_FILE).and_then(|bytes| TextFile::from_bytes(bytes).ok())
    }

    /// Everything of a map. The names are the ones of its place, in the eight languages.
    pub fn edit(&self, sd: Option<&SdCard>, zone: usize) -> Result<Edit, String> {
        let (settings, water) = self.settings(sd, zone)?;
        let area = settings.get(AREA) as usize;
        let (camera, cameras) = self.cameras(sd, area)?;
        let mut edit = Edit { zone, settings, water, area, camera, cameras, names: BTreeMap::new() };
        add_names(&self.name_files(sd), &mut edit);
        Ok(edit)
    }

    /// The files of the names of the places in the eight languages: the SD card first, then the game.
    pub fn name_files(&self, sd: Option<&SdCard>) -> Vec<Option<TextFile>> {
        (0..LANGUAGES.len()).map(|language| self.name_file(sd, language)).collect()
    }

    /// Writes the settings and the water of a map, in its file and in the table of all the maps.
    fn save_settings(&self, sd: &SdCard, zone: usize, settings: &Settings, water: &Water) -> Result<(), String> {
        let current = self.zone_pack(Some(sd), zone)?;
        let parts = bundle::split(&current)?.1;
        let (mut first, mut surfaces) = (parts[SETTINGS_PART].to_vec(), parts[WATER_PART].to_vec());
        first[..SETTINGS_SIZE].copy_from_slice(&settings.0);
        surfaces[..WATER_SIZE].copy_from_slice(&water.0);
        let pack = bundle::replace_part_with(&bundle::replace_part_with(&current, SETTINGS_PART, &first, 4)?, WATER_PART, &surfaces, 4)?;
        if pack != current {
            let game = self.zones.first_file(zone).ok_or("This map is not in the archive.")?;
            if pack == plain(game.to_vec())? {
                put_map_file(sd, "zone", zone, None)?;
            } else {
                let bytes = if lz::is_compressed(game) { lz::compress_lz11(&pack) } else { pack };
                put_map_file(sd, "zone", zone, Some(&bytes))?;
            }
        }
        // The table of all the maps: the game reads it when it starts.
        let number = self.table_file();
        let current = plain(Store::raw(&self.zones, Some(sd), "zone", number)?)?;
        let mut table = current.clone();
        let place = zone * SETTINGS_SIZE;
        table.get_mut(place..place + SETTINGS_SIZE).ok_or("The table of the maps is too short.")?.copy_from_slice(&settings.0);
        if table != current {
            let game = self.zones.first_file(number).ok_or("The table of the maps is not in the archive.")?;
            put_map_file(sd, "zone", number, (table != game).then_some(table.as_slice()))?;
        }
        Ok(())
    }

    /// Writes the camera that an area uses, and its cameras.
    fn save_cameras(&self, sd: &SdCard, area: usize, used: u16, cameras: &[Camera]) -> Result<(), String> {
        let (old_used, old_cameras) = self.cameras(Some(sd), area)?;
        if cameras != old_cameras.as_slice() {
            let current = plain(Store::raw(&self.areas, Some(sd), "area", area)?)?;
            let mut part = bundle::split(&current)?.1[CAMERA_PART].to_vec();
            for (place, camera) in cameras.iter().enumerate() {
                part[place * CAMERA_SIZE..(place + 1) * CAMERA_SIZE].copy_from_slice(&camera.0);
            }
            let pack = bundle::replace_part_with(&current, CAMERA_PART, &part, 4)?;
            let game = self.areas.first_file(area).ok_or("This area is not in the archive.")?;
            if pack == plain(game.to_vec())? {
                put_map_file(sd, "area", area, None)?;
            } else {
                let bytes = lz::compress_lz11(&pack);
                if bytes.len() > self.area_room {
                    return Err(format!("The file of the area {area} is too big for the game once compressed ({} KB).", bytes.len() / 1024));
                }
                put_map_file(sd, "area", area, Some(&bytes))?;
            }
        }
        if used != old_used {
            let number = self.area_count();
            let mut table = plain(Store::raw(&self.areas, Some(sd), "area", number)?)?;
            let at = area * AREA_ENTRY + AREA_CAMERA_AT;
            table.get_mut(at..at + 2).ok_or("The table of the areas is too short.")?.copy_from_slice(&used.to_le_bytes());
            let game = self.areas.first_file(number).ok_or("The table of the areas is not in the archive.")?;
            put_map_file(sd, "area", number, (table != game).then_some(table.as_slice()))?;
        }
        Ok(())
    }

    /// Writes the names that changed, in each language.
    fn save_names(&self, sd: &SdCard, names: &BTreeMap<(usize, usize), String>) -> Result<(), String> {
        for language in 0..LANGUAGES.len() {
            let Some(mut file) = self.name_file(Some(sd), language) else { continue };
            let mut changed = false;
            for (&(of, line), name) in names {
                if of == language && file.text(line).as_ref() != Some(name) {
                    file.set_text(line, name)?;
                    changed = true;
                }
            }
            if !changed {
                continue;
            }
            let path = text::sd_path(language, NAME_FILE);
            if Some(&file) == self.game_name_file(language).as_ref() {
                sd.remove(&path)?;
            } else {
                sd.write(&path, &file.to_bytes())?;
            }
            sd.mark_changed(&path)?;
        }
        Ok(())
    }

    /// Writes what the page edits for a map.
    pub fn save(&self, sd: &SdCard, edit: &Edit) -> Result<(), String> {
        self.save_names(sd, &edit.names)?;
        self.save_cameras(sd, edit.area, edit.camera, &edit.cameras)?;
        self.save_settings(sd, edit.zone, &edit.settings, &edit.water)
    }

    /// The maps that have a file on the SD card with settings or water that are not the ones of the game.
    pub fn changed_zones(&self, sd: Option<&SdCard>) -> Vec<usize> {
        let Some(sd) = sd else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(sd.overworld_folder().join("zone")) else { return Vec::new() };
        let mut found: Vec<usize> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str().and_then(|name| name.strip_suffix(".bin")).and_then(|n| n.parse::<usize>().ok()))
            .filter(|&zone| zone < self.zone_count() && self.settings(Some(sd), zone).ok() != self.settings(None, zone).ok())
            .collect();
        found.sort_unstable();
        found
    }

    /// The number of maps of the game that use an area.
    pub fn maps_of_area(&self, area: usize) -> usize {
        let Some(table) = self.zones.first_file(self.table_file()) else { return 0 };
        table.chunks_exact(SETTINGS_SIZE).filter(|zone| u16::from_le_bytes([zone[2], zone[3]]) as usize == area).count()
    }

    /// The settings of all the maps as a CSV file: one line for each map, one column for each value.
    pub fn to_csv(&self, sd: Option<&SdCard>) -> Result<String, String> {
        let mut out = format!("map,{}\n", FIELDS.map(|f| f.key).join(","));
        for zone in 0..self.zone_count() {
            let (settings, _) = self.settings(sd, zone)?;
            out.push_str(&format!("{zone},{}\n", FIELDS.map(|f| settings.get(f).to_string()).join(",")));
        }
        Ok(out)
    }

    /// Reads a CSV file of [`Store::to_csv`] (some columns and some lines are enough) and writes the maps that change. Gives their
    /// number. Nothing is written when a line is wrong.
    pub fn import_csv(&self, sd: &SdCard, csv: &str) -> Result<usize, String> {
        let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
        let head: Vec<&str> = lines.next().ok_or("The file is empty.")?.split(',').map(str::trim).collect();
        if head.first() != Some(&"map") {
            return Err("The first column of the file must be map.".into());
        }
        let columns = head[1..].iter().map(|key| FIELDS.iter().find(|f| f.key == *key).copied().ok_or_else(|| format!("There is no value {key}."))).collect::<Result<Vec<_>, _>>()?;
        let mut changes = Vec::new();
        for line in lines {
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            let wrong = || format!("The line \"{line}\" is wrong.");
            let zone: usize = cells.first().and_then(|c| c.parse().ok()).ok_or_else(wrong)?;
            if cells.len() != head.len() {
                return Err(wrong());
            }
            let (mut settings, water) = self.settings(Some(sd), zone)?;
            let before = settings;
            for (field, cell) in columns.iter().zip(&cells[1..]) {
                let value: i64 = cell.parse().map_err(|_| wrong())?;
                let (low, high) = field.range();
                if value < low || value > high {
                    return Err(format!("The map {zone}: {} must be from {low} to {high}.", field.key));
                }
                settings.set(*field, value);
            }
            if settings != before {
                if settings.get(AREA) as usize >= self.area_count() {
                    return Err(format!("The map {zone}: the game has no area {}.", settings.get(AREA)));
                }
                changes.push((zone, settings, water));
            }
        }
        for (zone, settings, water) in &changes {
            self.save_settings(sd, *zone, settings, water)?;
        }
        Ok(changes.len())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn setup(name: &str) -> Option<(Store, SdCard, std::path::PathBuf)> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        let folder = std::env::temp_dir().join(format!("sango_zone_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        Some((Store::open(&Dump::open(&root).unwrap()).unwrap(), sd, folder))
    }

    #[test]
    fn a_value_changes_only_its_bits() {
        let mut settings = Settings([0xFF; SETTINGS_SIZE]);
        settings.set(WEATHER, 3);
        assert_eq!((settings.get(WEATHER), settings.get(SKY), settings.get(BATTLE_BACKGROUND)), (3, 1, 127));
        settings.set(FLY, 0);
        assert_eq!((settings.get(FLY), settings.get(ESCAPE_ROPE), settings.get(BICYCLE_MUSIC)), (0, 1, 1));
        settings.set(FLY_Y, -5);
        assert_eq!((settings.get(FLY_Y), settings.get(FLY_X), settings.get(FLY_Z)), (-5, -1, -1));
        settings.set(FLY_MAP, 2000);
        assert_eq!(settings.get(FLY_MAP), 1023, "the value stays in its range");
    }

    /// With the dump: every map is read, and the table of all the maps has the same settings as the file of each map.
    #[test]
    fn every_map_is_read_and_the_table_agrees() {
        let Some((store, _, folder)) = setup("read") else { return };
        assert_eq!((store.zone_count(), store.area_count()), (536, 228));
        let table = store.zones.first_file(536).unwrap();
        for zone in 0..store.zone_count() {
            let edit = store.edit(None, zone).unwrap();
            assert_eq!(&edit.settings.0[..], &table[zone * SETTINGS_SIZE..(zone + 1) * SETTINGS_SIZE], "map {zone}");
            assert!((edit.camera as usize) < CAMERA_COUNT, "map {zone}: camera {}", edit.camera);
            assert_eq!(edit.cameras[0].value(36).round(), 254.0);
        }
        let littleroot = store.edit(None, 6).unwrap();
        assert!(store.name_file(None, 2).unwrap().text(littleroot.settings.get(PLACE_NAME) as usize).is_some_and(|name| !name.is_empty()));
        assert_eq!((littleroot.settings.get(FLY), littleroot.settings.get(BICYCLE), littleroot.settings.get(RUNNING)), (1, 1, 1));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: every part of a map is changed, saved and read again; the other parts of the files stay; the game values
    /// take the files off the card.
    #[test]
    fn a_map_is_changed_saved_and_reset() {
        let Some((store, sd, folder)) = setup("save") else { return };
        let game = store.edit(None, 6).unwrap();
        let mut edit = game.clone();
        edit.settings.set(FLY, 0);
        edit.settings.set(WEATHER, 1);
        edit.water.set_on(0, true);
        edit.water.set_height(0, -12);
        edit.camera = 3;
        edit.cameras[3].set_value(36, 500.0);
        let line = edit.settings.get(PLACE_NAME) as usize;
        edit.names.insert((2, line), "Test Town".to_string());
        store.save(&sd, &edit).unwrap();
        assert_eq!(store.edit(Some(&sd), 6).unwrap(), edit);
        assert_eq!(store.changed_zones(Some(&sd)), vec![6]);
        for file in ["zone/6.bin", "zone/536.bin", "zone/index.bin", "area/228.bin", "area/index.bin"] {
            assert!(sd.overworld_folder().join(file).exists(), "{file}");
        }
        assert!(sd.read(&text::sd_path(2, NAME_FILE)).is_some() && sd.read(&text::sd_path(3, NAME_FILE)).is_none());
        // The table has the new settings, and the other parts of the pack stay.
        let table = std::fs::read(sd.overworld_folder().join("zone/536.bin")).unwrap();
        assert_eq!(&table[6 * SETTINGS_SIZE..7 * SETTINGS_SIZE], &edit.settings.0[..]);
        let (new, old) = (store.zone_pack(Some(&sd), 6).unwrap(), store.zone_pack(None, 6).unwrap());
        let (new, old) = (bundle::split(&new).unwrap().1, bundle::split(&old).unwrap().1);
        assert!((1..4).all(|part| new[part] == old[part]));
        // The values of the game again.
        let mut back = game.clone();
        let game_name = game.names[&(2, line)].clone();
        back.names.insert((2, line), game_name);
        store.save(&sd, &back).unwrap();
        assert_eq!(store.edit(Some(&sd), 6).unwrap(), game);
        for file in ["zone/6.bin", "zone/536.bin", "zone/index.bin", "area/228.bin", "area/index.bin"] {
            assert!(!sd.overworld_folder().join(file).exists(), "{file}");
        }
        assert!(sd.read(&text::sd_path(2, NAME_FILE)).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: the CSV file of all the maps goes out and comes back.
    #[test]
    fn the_csv_file_of_the_maps_is_read_again() {
        let Some((store, sd, folder)) = setup("csv") else { return };
        let csv = store.to_csv(None).unwrap();
        assert_eq!(csv.lines().count(), 537);
        assert_eq!(store.import_csv(&sd, &csv), Ok(0), "nothing changes");
        let changes = "map,fly,bicycle\n6,0,0\n7,0,0\n";
        assert_eq!(store.import_csv(&sd, changes), Ok(2));
        assert_eq!(store.import_csv(&sd, changes), Ok(0), "the same file again changes nothing");
        let (settings, _) = store.settings(Some(&sd), 6).unwrap();
        assert_eq!((settings.get(FLY), settings.get(BICYCLE)), (0, 0));
        assert!(store.import_csv(&sd, "map,fly\n6,5\n").is_err(), "a value out of its range");
        assert!(store.import_csv(&sd, "map,unknown\n6,1\n").is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
