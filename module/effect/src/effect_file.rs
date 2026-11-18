//! A particle effect file of the game (the archive 31): the layers of the effect, and what the user can change in each one.
//!
//! The file has the signature `CGFX`. Its parts point at each other with offsets that count from the place of the offset itself. A *layer* is an
//! emitter (how many particles, when, in which shape) with the set of particles that it makes (their colour, their size, their life). The
//! page changes numbers in place: the size of the file never changes.

/// The signature at the start of the file.
const MAGIC: &[u8; 4] = b"CGFX";
/// The signatures of an emitter and of a set of particles, at the byte 4 of the object.
const EMITTER_SIGNATURE: &[u8; 4] = b"PEMT";
const SET_SIGNATURE: &[u8; 4] = b"CNOD";
/// The least bytes of an emitter and of a set.
const EMITTER_SIZE: usize = 0xE0;
const SET_SIZE: usize = 0x60;

// The places in an emitter.
const NAME: usize = 0x0C;
const SET_PATH: usize = 0xB8;
const FORM: usize = 0xBC;
const RATIO: usize = 0xC4;
const RATIO_RANDOM: usize = 0xC8;
const INTERVAL: usize = 0xCC;
const INTERVAL_RANDOM: usize = 0xD0;
const INFINITE: usize = 0xD4;
const SPAN: usize = 0xD8;
const START: usize = 0xDC;

// The places in a set of particles.
const SET_NAME: usize = 0x0C;
const COLLECTION: usize = 0x30;
const INITIALIZER_COUNT: usize = 0x34;
const INITIALIZER_TABLE: usize = 0x38;
const UPDATER_COUNT: usize = 0x3C;
const UPDATER_TABLE: usize = 0x40;
const MAX_ENTRIES: usize = 32;

// The places in an initializer or an updater.
const OBJECT_TYPE: usize = 0;
const OBJECT_TARGET: usize = 8;

/// What a rule changes in the particles.
const TARGET_SIZE: u32 = 1;
const TARGET_COLOR: u32 = 3;
const TARGET_ALPHA: u32 = 4;
const TARGET_SIZE_EXTRA: u32 = 8;
const TARGET_LIFE: u32 = 11;

const COLOR_INITIALIZER: u32 = 0x0400_0000;
const LIFE_INITIALIZER: u32 = 0x0100_0000;
const FIXED_VECTOR: u32 = 0x0080_0001;
const KEYED_VECTOR: u32 = 0x0010_0000;
const FIXED_NUMBER: u32 = 0x0040_0001;
const KEYED_NUMBER: u32 = 0x0020_0000;
/// The times of a curve are in 16.16 fixed point.
const TIME_SCALE: f32 = 65536.0;

/// The shape of the area where an emitter makes its particles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    Cube,
    Cylinder,
    Disc,
    Point,
    Rectangle,
    Sphere,
}

impl FormKind {
    fn from_word(word: u32) -> Option<FormKind> {
        Some(match word {
            0x8000_0000 => FormKind::Cube,
            0x4000_0000 => FormKind::Cylinder,
            0x2000_0000 => FormKind::Disc,
            0x1000_0000 => FormKind::Point,
            0x0800_0000 => FormKind::Rectangle,
            0x0400_0000 => FormKind::Sphere,
            _ => return None,
        })
    }

    /// The name that the user sees.
    pub fn name(self) -> &'static str {
        match self {
            FormKind::Cube => "Box",
            FormKind::Cylinder => "Cylinder",
            FormKind::Disc => "Disc",
            FormKind::Point => "Point",
            FormKind::Rectangle => "Rectangle",
            FormKind::Sphere => "Sphere",
        }
    }
}

/// The kind of a number of a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Number,
    Angle,
    Vec2,
    Vec3,
    Switch,
}

/// A number of a shape: what the user sees, the place and the kind.
#[derive(Clone, Copy, Debug)]
pub struct FormField {
    pub name: &'static str,
    pub help: &'static str,
    pub at: usize,
    pub kind: FieldKind,
}

const INNER: FormField = FormField { name: "Empty part", help: "The part of the shape in the middle where no particle appears (0 to 1).", at: 8, kind: FieldKind::Number };
const EVEN: FormField = FormField { name: "Even spread", help: "The particles are spread evenly, not at random.", at: 0, kind: FieldKind::Switch };

/// The numbers of the shape of a kind.
pub fn form_fields(kind: FormKind) -> Vec<FormField> {
    let angles = |at: usize| {
        vec![
            FormField { name: "Arc", help: "How much of the circle is used.", at, kind: FieldKind::Angle },
            FormField { name: "Arc start", help: "Where the arc starts.", at: at + 4, kind: FieldKind::Angle },
            FormField { name: "Arc swing", help: "How much the arc moves.", at: at + 8, kind: FieldKind::Angle },
        ]
    };
    match kind {
        FormKind::Point => Vec::new(),
        FormKind::Cube => vec![INNER, FormField { name: "Size", help: "The size of the box on each axis.", at: 0x0C, kind: FieldKind::Vec3 }, FormField { at: 0x18, ..EVEN }],
        FormKind::Rectangle => vec![INNER, FormField { name: "Size", help: "The size of the rectangle.", at: 0x0C, kind: FieldKind::Vec2 }, FormField { at: 0x14, ..EVEN }],
        FormKind::Disc => {
            let mut list = vec![INNER];
            list.extend(angles(0x0C));
            list.push(FormField { name: "Size", help: "The size of the disc.", at: 0x18, kind: FieldKind::Vec2 });
            list.push(FormField { name: "Fixed start", help: "The arc does not move.", at: 0x20, kind: FieldKind::Switch });
            list.push(FormField { at: 0x21, ..EVEN });
            list
        }
        FormKind::Cylinder | FormKind::Sphere => {
            let mut list = vec![INNER];
            list.extend(angles(0x0C));
            list.push(FormField { name: "Size", help: "The size of the shape on each axis.", at: 0x18, kind: FieldKind::Vec3 });
            list.push(FormField { name: "Fixed start", help: "The arc does not move.", at: 0x24, kind: FieldKind::Switch });
            list.push(FormField { at: 0x25, ..EVEN });
            list
        }
    }
}

/// How an emitter makes its particles in time.
#[derive(Clone, Debug, PartialEq)]
pub struct Emission {
    /// The number of particles at each emission.
    pub ratio: f32,
    pub ratio_random: f32,
    /// The frames between two emissions.
    pub interval: i32,
    pub interval_random: f32,
    /// True when the emitter never stops.
    pub infinite: bool,
    /// The frames that the emitter works, when it stops.
    pub span: i32,
    /// The frame where the emitter starts.
    pub start: i32,
}

/// A colour that changes with the life of the particle: at the start, in the middle and at the end, and the times of the changes (0 to 1).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorCurve {
    pub fixed: bool,
    pub start: [f32; 3],
    pub middle: [f32; 3],
    pub end: [f32; 3],
    /// The moment where the middle colour is reached, and the moment where the change to the end colour starts.
    pub in_time: f32,
    pub out_time: f32,
}

/// A number that changes with the life of the particle (the transparency): at the start, at the peak and at the end.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberCurve {
    pub fixed: bool,
    pub start: f32,
    pub peak: f32,
    pub end: f32,
    pub in_time: f32,
    pub out_time: f32,
}

/// A picture of the effect (the texture of the particles): where it is and how it is stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub name: String,
    /// The format of the graphics chip (see `sango_core::model::decode_texture`).
    pub format: u32,
    pub width: usize,
    pub height: usize,
    /// The number of sizes of the picture (the first one is the picture itself, the others are smaller copies).
    pub levels: usize,
    /// The place of the bytes of the picture in the file, and their number.
    at: usize,
    size: usize,
}

// The places in a texture object.
const TEXTURE_SIGNATURE: &[u8; 4] = b"TXOB";
const TEXTURE_LEVELS: usize = 0x28;
const TEXTURE_FORMAT: usize = 0x34;
const TEXTURE_HEIGHT: usize = 0x3C;
const TEXTURE_WIDTH: usize = 0x40;
const TEXTURE_SIZE: usize = 0x44;
const TEXTURE_DATA: usize = 0x48;

/// A file of an effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    data: Vec<u8>,
    emitters: Vec<usize>,
    sets: Vec<usize>,
    pictures: Vec<Picture>,
}

fn rel_at(data: &[u8], at: usize) -> Option<usize> {
    let bytes = data.get(at..at + 4)?;
    let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let target = at as i64 + i64::from(value);
    (target >= 0 && (target as usize) < data.len()).then_some(target as usize)
}

impl Effect {
    /// Reads a file.
    pub fn parse(bytes: &[u8]) -> Result<Effect, String> {
        if bytes.len() < 0x20 || &bytes[..4] != MAGIC {
            return Err("This is not an effect file.".to_string());
        }
        let (mut emitters, mut sets) = (Vec::new(), Vec::new());
        let mut at = 0;
        while at + EMITTER_SIZE.max(SET_SIZE) <= bytes.len() {
            match &bytes[at + 4..at + 8] {
                s if s == EMITTER_SIGNATURE => emitters.push(at),
                s if s == SET_SIGNATURE => sets.push(at),
                _ => {}
            }
            at += 4;
        }
        let mut effect = Effect { data: bytes.to_vec(), emitters, sets, pictures: Vec::new() };
        effect.pictures = effect.find_pictures();
        Ok(effect)
    }

    /// The pictures of the effect: the objects that have a size, a format and bytes inside the file.
    fn find_pictures(&self) -> Vec<Picture> {
        let mut found = Vec::new();
        let mut at = 0;
        while at + TEXTURE_DATA + 4 <= self.data.len() {
            if &self.data[at + 4..at + 8] == TEXTURE_SIGNATURE {
                let (format, width, height) = (self.u32_at(at + TEXTURE_FORMAT), self.u32_at(at + TEXTURE_WIDTH) as usize, self.u32_at(at + TEXTURE_HEIGHT) as usize);
                let (size, levels) = (self.u32_at(at + TEXTURE_SIZE) as usize, self.u32_at(at + TEXTURE_LEVELS) as usize);
                if let Some(start) = rel_at(&self.data, at + TEXTURE_DATA) {
                    if format <= 13 && width >= 8 && height >= 8 && width % 8 == 0 && height % 8 == 0 && levels >= 1 && start + size <= self.data.len() && size >= sango_core::model::texture_size(format, width, height) {
                        found.push(Picture { name: self.text_at(at + NAME), format, width, height, levels, at: start, size });
                    }
                }
            }
            at += 4;
        }
        found
    }

    pub fn pictures(&self) -> &[Picture] {
        &self.pictures
    }

    /// The first size of a picture as RGBA pixels, the first row at the top.
    pub fn picture_rgba(&self, picture: usize) -> Option<Vec<u8>> {
        let p = self.pictures.get(picture)?;
        sango_core::model::decode_texture(p.format, p.width, p.height, &self.data[p.at..p.at + p.size])
    }

    /// Replaces the first size of a picture by RGBA pixels (the first row at the top): they are written in the format of the picture, so the file
    /// keeps its size. The picture must have the size of the old one.
    pub fn set_picture_rgba(&mut self, picture: usize, rgba: &[u8]) -> Result<(), String> {
        let p = self.pictures.get(picture).ok_or("This picture is not in the effect.")?.clone();
        if rgba.len() != p.width * p.height * 4 {
            return Err(format!("The picture must have {} x {} pixels.", p.width, p.height));
        }
        if p.levels > 1 {
            return Err("This picture has smaller copies of itself: it cannot be replaced.".to_string());
        }
        let bytes = sango_core::model::encode_texture(p.format, p.width, p.height, rgba).ok_or("The format of this picture cannot be written.")?;
        self.data[p.at..p.at + bytes.len()].copy_from_slice(&bytes);
        Ok(())
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes([self.data[at], self.data[at + 1], self.data[at + 2], self.data[at + 3]])
    }
    fn f32_at(&self, at: usize) -> f32 {
        f32::from_bits(self.u32_at(at))
    }
    fn put_u32(&mut self, at: usize, value: u32) {
        self.data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put_f32(&mut self, at: usize, value: f32) {
        self.put_u32(at, value.to_bits());
    }
    fn text_at(&self, at: usize) -> String {
        let Some(start) = rel_at(&self.data, at) else { return String::new() };
        let end = self.data[start..].iter().position(|&b| b == 0).map_or(self.data.len(), |n| start + n);
        String::from_utf8_lossy(&self.data[start..end]).to_string()
    }

    /// The number of layers: one for each emitter.
    pub fn layer_count(&self) -> usize {
        self.emitters.len()
    }

    /// The name of an emitter.
    pub fn emitter_name(&self, layer: usize) -> String {
        self.text_at(self.emitters[layer] + NAME)
    }

    /// The name of the set of particles that a layer makes.
    pub fn set_name(&self, layer: usize) -> String {
        self.text_at(self.emitters[layer] + SET_PATH)
    }

    /// The set of particles of a layer (the place in the file), when it has one.
    fn set_of(&self, layer: usize) -> Option<usize> {
        let wanted = self.set_name(layer);
        self.sets.iter().copied().find(|&set| self.text_at(set + SET_NAME) == wanted)
    }

    pub fn emission(&self, layer: usize) -> Emission {
        let at = self.emitters[layer];
        Emission { ratio: self.f32_at(at + RATIO), ratio_random: self.f32_at(at + RATIO_RANDOM), interval: self.u32_at(at + INTERVAL) as i32, interval_random: self.f32_at(at + INTERVAL_RANDOM), infinite: self.data[at + INFINITE] != 0, span: self.u32_at(at + SPAN) as i32, start: self.u32_at(at + START) as i32 }
    }

    pub fn set_emission(&mut self, layer: usize, emission: &Emission) {
        let at = self.emitters[layer];
        self.put_f32(at + RATIO, emission.ratio.max(0.0));
        self.put_f32(at + RATIO_RANDOM, emission.ratio_random.max(0.0));
        self.put_u32(at + INTERVAL, emission.interval.max(1) as u32);
        self.put_f32(at + INTERVAL_RANDOM, emission.interval_random.max(0.0));
        self.data[at + INFINITE] = u8::from(emission.infinite);
        self.put_u32(at + SPAN, emission.span.max(0) as u32);
        self.put_u32(at + START, emission.start.max(0) as u32);
    }

    /// The place of the shape of a layer.
    fn form_at(&self, layer: usize) -> Option<usize> {
        rel_at(&self.data, self.emitters[layer] + FORM).filter(|&at| at + 0x30 <= self.data.len())
    }

    /// The kind of the shape of a layer.
    pub fn form_kind(&self, layer: usize) -> Option<FormKind> {
        FormKind::from_word(self.u32_at(self.form_at(layer)?))
    }

    /// The numbers of the shape: for each field of [`form_fields`], its values (one, two or three numbers; a switch is 0 or 1).
    pub fn form_values(&self, layer: usize) -> Vec<(FormField, [f32; 3])> {
        let (Some(at), Some(kind)) = (self.form_at(layer), self.form_kind(layer)) else { return Vec::new() };
        form_fields(kind)
            .into_iter()
            .map(|field| {
                let place = at + field.at;
                let values = match field.kind {
                    FieldKind::Number | FieldKind::Angle => [self.f32_at(place), 0.0, 0.0],
                    FieldKind::Vec2 => [self.f32_at(place), self.f32_at(place + 4), 0.0],
                    FieldKind::Vec3 => [self.f32_at(place), self.f32_at(place + 4), self.f32_at(place + 8)],
                    FieldKind::Switch => [f32::from(self.data[place]), 0.0, 0.0],
                };
                (field, values)
            })
            .collect()
    }

    pub fn set_form_value(&mut self, layer: usize, field: &FormField, values: [f32; 3]) {
        let Some(at) = self.form_at(layer) else { return };
        let place = at + field.at;
        match field.kind {
            FieldKind::Number | FieldKind::Angle => self.put_f32(place, values[0]),
            FieldKind::Vec2 => {
                self.put_f32(place, values[0]);
                self.put_f32(place + 4, values[1]);
            }
            FieldKind::Vec3 => (0..3).for_each(|k| self.put_f32(place + 4 * k, values[k])),
            FieldKind::Switch => self.data[place] = u8::from(values[0] != 0.0),
        }
    }

    /// The initializers and the updaters of the set of a layer, with their place.
    fn rules(&self, layer: usize) -> (Vec<usize>, Vec<usize>) {
        let Some(set) = self.set_of(layer) else { return (Vec::new(), Vec::new()) };
        let table = |count_at: usize, table_at: usize| -> Vec<usize> {
            let count = self.u32_at(set + count_at) as usize;
            let Some(base) = rel_at(&self.data, set + table_at) else { return Vec::new() };
            if count > MAX_ENTRIES || base + 4 * count > self.data.len() {
                return Vec::new();
            }
            (0..count).filter_map(|k| rel_at(&self.data, base + 4 * k)).filter(|&at| at + 16 <= self.data.len()).collect()
        };
        (table(INITIALIZER_COUNT, INITIALIZER_TABLE), table(UPDATER_COUNT, UPDATER_TABLE))
    }

    fn vec3_at(&self, at: usize) -> [f32; 3] {
        [self.f32_at(at), self.f32_at(at + 4), self.f32_at(at + 8)]
    }
    fn put_vec3(&mut self, at: usize, value: [f32; 3]) {
        (0..3).for_each(|k| self.put_f32(at + 4 * k, value[k]));
    }

    /// The updater of a target (the colour, the transparency, the size).
    fn updater(&self, layer: usize, target: u32) -> Option<usize> {
        self.rules(layer).1.into_iter().find(|&at| at + 84 <= self.data.len() && self.u32_at(at + OBJECT_TARGET) == target)
    }

    /// The colour of the particles of a layer, as it changes with their life.
    pub fn color(&self, layer: usize) -> Option<ColorCurve> {
        if let Some(at) = self.updater(layer, TARGET_COLOR) {
            return match self.u32_at(at + OBJECT_TYPE) {
                FIXED_VECTOR => {
                    let value = self.vec3_at(at + 16);
                    Some(ColorCurve { fixed: true, start: value, middle: value, end: value, in_time: 0.0, out_time: 1.0 })
                }
                KEYED_VECTOR => {
                    let (slope, offset) = (self.vec3_at(at + 60), self.vec3_at(at + 72));
                    Some(ColorCurve { fixed: false, start: self.vec3_at(at + 24), middle: self.vec3_at(at + 36), end: [offset[0] + slope[0], offset[1] + slope[1], offset[2] + slope[2]], in_time: self.u32_at(at + 16) as f32 / TIME_SCALE, out_time: self.u32_at(at + 20) as f32 / TIME_SCALE })
                }
                _ => None,
            };
        }
        let initializer = self.rules(layer).0.into_iter().find(|&at| self.u32_at(at + OBJECT_TARGET) == TARGET_COLOR && self.u32_at(at + OBJECT_TYPE) == COLOR_INITIALIZER)?;
        let value = self.vec3_at(initializer + 12);
        Some(ColorCurve { fixed: true, start: value, middle: value, end: value, in_time: 0.0, out_time: 1.0 })
    }

    /// Writes the colour of the particles of a layer.
    pub fn set_color(&mut self, layer: usize, color: &ColorCurve) {
        if let Some(at) = self.updater(layer, TARGET_COLOR) {
            match self.u32_at(at + OBJECT_TYPE) {
                FIXED_VECTOR => self.put_vec3(at + 16, color.middle),
                KEYED_VECTOR => self.write_vector_curve(at, color.start, color.middle, color.end, color.in_time, color.out_time),
                _ => {}
            }
        } else if let Some(initializer) = self.rules(layer).0.into_iter().find(|&at| self.u32_at(at + OBJECT_TARGET) == TARGET_COLOR && self.u32_at(at + OBJECT_TYPE) == COLOR_INITIALIZER) {
            self.put_vec3(initializer + 12, color.start);
        }
    }

    /// Writes a curve of three numbers (the start, the middle and the end) in the form that the game reads: the values and the slopes.
    fn write_vector_curve(&mut self, at: usize, start: [f32; 3], middle: [f32; 3], end: [f32; 3], in_time: f32, out_time: f32) {
        let in_time = in_time.clamp(0.0, 1.0);
        let out_time = out_time.clamp(in_time, 1.0);
        let out_span = 1.0 - out_time;
        let slope = |from: [f32; 3], to: [f32; 3], span: f32| -> [f32; 3] { if span > 0.0 { [0, 1, 2].map(|k| (to[k] - from[k]) / span) } else { [0.0; 3] } };
        let in_slope = slope(start, middle, in_time);
        let out_slope = slope(middle, end, out_span);
        let out_offset = [0, 1, 2].map(|k| middle[k] - out_slope[k] * out_time);
        self.put_u32(at + 16, (in_time * TIME_SCALE).round() as u32);
        self.put_u32(at + 20, (out_time * TIME_SCALE).round() as u32);
        self.put_vec3(at + 24, start);
        self.put_vec3(at + 36, middle);
        self.put_vec3(at + 48, in_slope);
        self.put_vec3(at + 60, out_slope);
        self.put_vec3(at + 72, out_offset);
    }

    /// The transparency of the particles of a layer (0 is clear, 1 is solid).
    pub fn alpha(&self, layer: usize) -> Option<NumberCurve> {
        let at = self.updater(layer, TARGET_ALPHA)?;
        let fixed = |raw: i32| raw as f32 / TIME_SCALE;
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_NUMBER => {
                let value = self.f32_at(at + 16);
                Some(NumberCurve { fixed: true, start: value, peak: value, end: value, in_time: 0.0, out_time: 1.0 })
            }
            KEYED_NUMBER => {
                let raw = |k: usize| self.u32_at(at + 24 + 4 * k) as i32;
                Some(NumberCurve { fixed: false, start: fixed(raw(0)), peak: fixed(raw(1)), end: fixed(raw(4)) + fixed(raw(3)), in_time: self.u32_at(at + 16) as f32 / TIME_SCALE, out_time: self.u32_at(at + 20) as f32 / TIME_SCALE })
            }
            _ => None,
        }
    }

    pub fn set_alpha(&mut self, layer: usize, curve: &NumberCurve) {
        let Some(at) = self.updater(layer, TARGET_ALPHA) else { return };
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_NUMBER => self.put_f32(at + 16, curve.peak),
            KEYED_NUMBER => {
                let in_time = curve.in_time.clamp(0.0, 1.0);
                let out_time = curve.out_time.clamp(in_time, 1.0);
                let fix = |value: f32| (value * TIME_SCALE + if value >= 0.0 { 0.5 } else { -0.5 }) as i32 as u32;
                let in_slope = if in_time > 0.0 { (curve.peak - curve.start) / in_time } else { 0.0 };
                let out_slope = if out_time < 1.0 { (curve.end - curve.peak) / (1.0 - out_time) } else { 0.0 };
                let out_offset = curve.peak - out_slope * out_time;
                self.put_u32(at + 16, (in_time * TIME_SCALE).round() as u32);
                self.put_u32(at + 20, (out_time * TIME_SCALE).round() as u32);
                for (k, value) in [curve.start, curve.peak, in_slope, out_slope, out_offset].into_iter().enumerate() {
                    self.put_u32(at + 24 + 4 * k, fix(value));
                }
            }
            _ => {}
        }
    }

    /// The updater that gives the size of the particles: the extra size when the set has one, else the size.
    fn size_updater(&self, layer: usize) -> Option<usize> {
        self.updater(layer, TARGET_SIZE_EXTRA).or_else(|| self.updater(layer, TARGET_SIZE))
    }

    /// The size of the particles of a layer, at the start, in the middle and at the end of their life.
    pub fn size(&self, layer: usize) -> Option<ColorCurve> {
        let at = self.size_updater(layer)?;
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_VECTOR => {
                let value = self.vec3_at(at + 16);
                Some(ColorCurve { fixed: true, start: value, middle: value, end: value, in_time: 0.0, out_time: 1.0 })
            }
            KEYED_VECTOR => {
                let (slope, offset) = (self.vec3_at(at + 60), self.vec3_at(at + 72));
                Some(ColorCurve { fixed: false, start: self.vec3_at(at + 24), middle: self.vec3_at(at + 36), end: [offset[0] + slope[0], offset[1] + slope[1], offset[2] + slope[2]], in_time: self.u32_at(at + 16) as f32 / TIME_SCALE, out_time: self.u32_at(at + 20) as f32 / TIME_SCALE })
            }
            _ => None,
        }
    }

    pub fn set_size(&mut self, layer: usize, size: &ColorCurve) {
        let Some(at) = self.size_updater(layer) else { return };
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_VECTOR => self.put_vec3(at + 16, size.middle),
            KEYED_VECTOR => self.write_vector_curve(at, size.start, size.middle, size.end, size.in_time, size.out_time),
            _ => {}
        }
    }

    /// The life of the particles of a layer, in frames: the shortest and the longest.
    pub fn life(&self, layer: usize) -> Option<(f32, f32)> {
        let at = self.rules(layer).0.into_iter().find(|&at| self.u32_at(at + OBJECT_TARGET) == TARGET_LIFE && self.u32_at(at + OBJECT_TYPE) == LIFE_INITIALIZER)?;
        Some((self.f32_at(at + 16), self.f32_at(at + 12)))
    }

    pub fn set_life(&mut self, layer: usize, shortest: f32, longest: f32) {
        let Some(at) = self.rules(layer).0.into_iter().find(|&at| self.u32_at(at + OBJECT_TARGET) == TARGET_LIFE && self.u32_at(at + OBJECT_TYPE) == LIFE_INITIALIZER) else { return };
        let (shortest, longest) = (shortest.max(0.0), longest.max(shortest.max(0.0)));
        self.put_f32(at + 12, longest);
        self.put_f32(at + 16, shortest);
    }

    /// The most particles that the set of a layer can have alive at once.
    pub fn capacity(&self, layer: usize) -> Option<i32> {
        let set = self.set_of(layer)?;
        let collection = rel_at(&self.data, set + COLLECTION)?;
        Some(self.u32_at(collection) as i32)
    }

    pub fn set_capacity(&mut self, layer: usize, capacity: i32) {
        if let Some(collection) = self.set_of(layer).and_then(|set| rel_at(&self.data, set + COLLECTION)) {
            self.put_u32(collection, capacity.max(1) as u32);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: every effect file is read, the numbers of its layers make sense, and a file that is not changed keeps its bytes.
    #[test]
    fn the_effects_of_the_game_are_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(31)).unwrap();
        let (mut layers, mut with_form, mut with_color, mut with_alpha, mut with_life) = (0, 0, 0, 0, 0);
        for id in 0..garc.file_count() {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let effect = Effect::parse(&bytes).unwrap_or_else(|e| panic!("file {id}: {e}"));
            assert_eq!(effect.bytes(), bytes.as_slice());
            for layer in 0..effect.layer_count() {
                layers += 1;
                let emission = effect.emission(layer);
                assert!(emission.ratio.is_finite() && emission.ratio >= 0.0 && emission.ratio < 100_000.0, "file {id} layer {layer}: {emission:?}");
                with_form += usize::from(effect.form_kind(layer).is_some());
                with_color += usize::from(effect.color(layer).is_some());
                with_alpha += usize::from(effect.alpha(layer).is_some());
                with_life += usize::from(effect.life(layer).is_some());
            }
        }
        assert!(layers > 2000, "{layers} layers");
        assert!(with_form * 10 > layers * 9, "shapes: {with_form} of {layers}");
        assert!(with_color * 4 > layers, "colours: {with_color} of {layers}");
        let mut formats = std::collections::BTreeMap::new();
        let mut undecoded = 0;
        for id in 0..garc.file_count() {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let effect = Effect::parse(&bytes).unwrap();
            for (place, picture) in effect.pictures().iter().enumerate() {
                *formats.entry((picture.format, picture.levels)).or_insert(0) += 1;
                undecoded += usize::from(effect.picture_rgba(place).is_none());
            }
        }
        eprintln!("pictures by (format, levels) {formats:?}, not decoded {undecoded}");
        eprintln!("layers {layers}, shapes {with_form}, colours {with_color}, transparency {with_alpha}, life {with_life}");
    }

    /// With the dump: the pictures of an effect are written as they are read, and a changed picture is read back.
    #[test]
    fn a_picture_is_replaced_and_read_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(31)).unwrap();
        let bytes = garc.first_file_decompressed(723).unwrap();
        let mut effect = Effect::parse(&bytes).unwrap();
        assert!(!effect.pictures().is_empty(), "the pictures of the effect");
        let old = effect.picture_rgba(0).unwrap();
        let p = effect.pictures()[0].clone();
        // The same pixels written again keep the file almost as it was (the format may hold fewer shades).
        effect.set_picture_rgba(0, &old).unwrap();
        assert_eq!(effect.bytes().len(), bytes.len());
        // A white picture with a solid centre is read back.
        let white: Vec<u8> = (0..p.width * p.height).flat_map(|n| [255, 255, 255, if (n % p.width) < p.width / 2 { 255 } else { 0 }]).collect();
        effect.set_picture_rgba(0, &white).unwrap();
        let again = effect.picture_rgba(0).unwrap();
        assert_eq!(again[3], 255, "the left half is solid");
        assert_eq!(again[(p.width - 1) * 4 + 3], 0, "the right half is clear");
        assert!(effect.set_picture_rgba(0, &white[4..]).is_err());
    }

    /// With the dump: a change of the emission, the shape, the colour and the life is written and read again.
    #[test]
    fn a_layer_is_changed_and_read_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(31)).unwrap();
        let bytes = garc.first_file_decompressed(0).unwrap();
        let mut effect = Effect::parse(&bytes).unwrap();
        let mut emission = effect.emission(0);
        emission.ratio = 3.5;
        emission.span = 99;
        effect.set_emission(0, &emission);
        assert_eq!(effect.emission(0), emission);
        let values = effect.form_values(0);
        assert!(!values.is_empty(), "the shape of the first layer");
        let (field, mut value) = (values[0].0, values[0].1);
        value[0] = 0.5;
        effect.set_form_value(0, &field, value);
        assert_eq!(effect.form_values(0)[0].1[0], 0.5);
        if let Some(mut color) = effect.color(0) {
            color.start = [1.0, 0.0, 0.0];
            color.middle = [0.0, 1.0, 0.0];
            color.end = [0.0, 0.0, 1.0];
            effect.set_color(0, &color);
            let again = effect.color(0).unwrap();
            if !color.fixed {
                assert!((again.start[0] - 1.0).abs() < 1e-4 && (again.end[2] - 1.0).abs() < 1e-3, "{again:?}");
            }
        }
        assert_eq!(effect.bytes().len(), bytes.len(), "the size of the file does not change");
        assert_eq!(Effect::parse(effect.bytes()).unwrap().emission(0), emission);
    }
}
