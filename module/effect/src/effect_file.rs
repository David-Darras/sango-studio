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
/// The signature of a material, at the byte 4 of the object.
const MATERIAL_SIGNATURE: &[u8; 4] = b"MTOB";
// The places in a material (its colours, the blend function, the first texture coordinates, the first texture mapper and
// the steps that mix the colours).
const MATERIAL_COLOR_NUMBERS: usize = 0x74;
const MATERIAL_CONSTANTS: usize = 0xE8;
const MATERIAL_BLEND: usize = 0x148;
const MATERIAL_COORDINATES: usize = 0x16C;
const MATERIAL_MAPPERS: usize = 0x274;
const MATERIAL_MIX: usize = 0x288;
/// In the steps of a material: the six steps (the number of their colour, then 6 words of commands each), the test of the
/// opacity and the commands of the saved colour.
const MIX_STEPS: usize = 0x2C;
const MIX_ALPHA_TEST: usize = 0xD4;
const MIX_BUFFER: usize = 0xDC;
/// A texture that only names another one of the file: the place of the name.
const REFERENCE_NAME: usize = 0x18;
// The places in a set of the shape builder, of the order of drawing and of the inheritance of the speed.
const SET_BUILDER: usize = 0x44;
const SET_SHAPE_INDEX: usize = 0x48;
const SET_PRIORITY: usize = 0x4C;
const SET_INHERIT_SPEED: usize = 0x52;
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
const OPACITY_INITIALIZER: u32 = 0x0200_0000;
const SIZE_INITIALIZER: u32 = 0x0020_0000;
// The places of the scale, the turn and the position of an emitter, in the object of the emitter.
const EMITTER_SCALE: usize = 0x30;
const EMITTER_ROTATION: usize = 0x3C;
const EMITTER_POSITION: usize = 0x48;
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

/// A rule that gives their first value to the streams of a new particle. The target is the stream: 0 the place, 1 the size, 2 the turn,
/// 3 the colour, 4 the transparency, 11 the life, 12 the speed.
#[derive(Clone, Debug, PartialEq)]
pub enum Initializer {
    /// Adds a speed in a direction.
    Directional { power: f32, direction: [f32; 3] },
    /// Adds a speed in a direction that changes by a random angle (a cone).
    RandomDirectional { power: f32, direction: [f32; 3], angle: f32 },
    /// Adds a speed away from the centre of the emitter.
    Origin { power: f32 },
    /// Adds a speed in a random direction.
    Random { power: f32 },
    /// Adds a speed away from the vertical axis of the emitter.
    YAxis { power: f32 },
    Vector3 { value: [f32; 3] },
    /// A number and a part of it that is changed at random.
    Float { base: f32, random: f32 },
    /// A whole number between a shortest and a longest value.
    FloatRange { max: f32, min: f32 },
    FloatImmediate { value: f32 },
    Vector2 { value: [f32; 2] },
    /// Three numbers that all change by the same random part.
    Vector3Random1 { base: [f32; 3], random: f32 },
    /// Three numbers that each change by their own random part.
    Vector3Random3 { base: [f32; 3], random: [f32; 3] },
    /// Multiplies the speed by a random number around 1.
    SpeedRandom { random: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct InitRule {
    pub target: i32,
    pub rule: Initializer,
}

/// A rule that changes the streams of a particle at each frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Updater {
    /// Multiplies the stream by a number at each frame (the air resistance when the stream is the speed).
    Acceleration { factor: f32 },
    /// Moves the particle by its speed.
    General,
    /// Adds a speed in a direction at each frame.
    Gravity { power: f32, direction: [f32; 3] },
    /// Turns the stream around an axis at each frame.
    Spin { power: f32, axis: [f32; 3] },
    /// Adds a random number to the stream at each frame.
    Random { power: [f32; 3], interval: i32 },
    /// Pulls the stream toward a place.
    Magnet { power: f32, position: [f32; 3], flags: u32 },
    Collision { power: f32, position: f32, flags: u32 },
    /// Adds a value to the stream at each frame (the turn of the picture). The value can follow a curve of the life.
    Add { value: [f32; 3], animation: Option<Animation> },
    AddRandom { value: [f32; 3] },
    Set { value: [f32; 3], animation: Option<Animation> },
    SetNumber { value: f32, animation: Option<Animation> },
    /// A number that follows 4 keys during the life (the transparency), in 65535 parts.
    NumberKeys { in_time: u32, out_time: u32, values: [i32; 5] },
    /// 3 numbers that follow 4 keys during the life (the colour, the size).
    VectorKeys { in_time: u32, out_time: u32, values: [[f32; 3]; 5] },
    SetVector2 { value: [f32; 2], animation: Option<Animation> },
    /// The picture is cut in a grid and the particle shows one cell, the number of which can follow a curve (the pictures of a flame or a bolt).
    TexturePattern { divisions: [u32; 2], index: i32, animation: Option<Animation> },
    /// The particle makes particles of another set: the set (a place in the sets), the shape where they appear (the place
    /// of its values in the file), when, and how many each time.
    Child { group: Option<usize>, shape: Option<usize>, timing: ChildTiming, count: i32 },
    /// The particle turns its up side along its speed, or along its last move.
    TurnToMove { along_speed: bool },
    /// A rule that the preview does not know, with its type.
    Other(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdRule {
    pub target: i32,
    pub rule: Updater,
}

/// When a particle makes its child particles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChildTiming {
    /// At its birth.
    Birth,
    /// At its death.
    Death,
    /// Every `interval` frames, between two parts of its life (0 to 1).
    LifeParts { start: f32, end: f32, interval: i32 },
    /// Every `interval` frames, between two frames of its life.
    Frames { start: i32, end: i32, interval: i32 },
}

/// How the frame of an animation comes from the life of a particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationKind {
    /// The frame is the number of frames since the birth.
    Frames,
    /// The animation is stretched over the life of the particle.
    Fitting,
    /// The animation repeats every `duration` frames, from a random start when `random_offset` is true.
    Loop { duration: u32, random_offset: bool },
    /// Each particle shows one random frame.
    Random,
}

/// A curve that a rule follows: the values of its numbers frame after frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// The number of frames, and the numbers of each frame.
    pub length: usize,
    pub stride: usize,
    /// For each number of the value, true when the curve gives it (else the value of the rule stays).
    pub enabled: Vec<bool>,
    pub data: Vec<f32>,
    pub kind: AnimationKind,
    /// The number of frames of the animation that the life covers.
    pub duration: f32,
}

/// How the game draws a particle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builder {
    /// A picture that faces the screen.
    Billboard,
    /// A picture that faces the screen, in the space of the world.
    WorldBillboard,
    /// A picture that stays upright and turns around the vertical axis to face the viewer.
    UpBillboard,
    /// A flat picture in the plane of the emitter.
    Plane,
}

/// The material of a set of particles: the steps that mix the picture with the colour of a particle, the way that the
/// result goes over what is behind, and the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// The six steps of the mix, with the colours of the material in them.
    pub stages: Vec<sango_core::model::Stage>,
    /// The colour that the steps can save.
    pub buffer_color: [u8; 4],
    /// The blend and the test of the opacity.
    pub state: sango_core::model::RenderState,
    /// The first two rows of the matrix of the coordinates of the picture: (u, v) of the particle to the ones of the
    /// picture, with the V axis going up.
    pub matrix: [[f32; 3]; 2],
    /// What happens outside of the picture, on each axis: 0 the edge goes on, 1 nothing, 2 the picture repeats, 3 it repeats turned over.
    pub wrap: [u32; 2],
    /// The picture, as a place in `Effect::pictures()`.
    pub picture: Option<usize>,
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
    /// The place of the object of the picture in the file (the materials point at it).
    object: usize,
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
    /// The materials: their places in the file.
    materials: Vec<usize>,
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
        let (mut emitters, mut sets, mut materials) = (Vec::new(), Vec::new(), Vec::new());
        let mut at = 0;
        while at + EMITTER_SIZE.max(SET_SIZE) <= bytes.len() {
            match &bytes[at + 4..at + 8] {
                s if s == EMITTER_SIGNATURE => emitters.push(at),
                s if s == SET_SIGNATURE => sets.push(at),
                s if s == MATERIAL_SIGNATURE => materials.push(at),
                _ => {}
            }
            at += 4;
        }
        let mut effect = Effect { data: bytes.to_vec(), emitters, sets, materials, pictures: Vec::new() };
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
                        found.push(Picture { name: self.text_at(at + NAME), format, width, height, levels, at: start, size, object: at });
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

    /// The number of sets of particles. A set can have no layer: the particles of another set make it.
    pub fn group_count(&self) -> usize {
        self.sets.len()
    }

    /// The set of particles that a layer makes.
    pub fn group_of(&self, layer: usize) -> Option<usize> {
        self.group_named(&self.set_name(layer))
    }

    fn group_named(&self, name: &str) -> Option<usize> {
        self.sets.iter().position(|&set| self.text_at(set + SET_NAME) == name)
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
        self.shape_kind(self.form_at(layer)?)
    }

    /// The kind of the shape whose values are at a place.
    pub fn shape_kind(&self, at: usize) -> Option<FormKind> {
        (at + 0x30 <= self.data.len()).then(|| FormKind::from_word(self.u32_at(at))).flatten()
    }

    /// The numbers of the shape: for each field of [`form_fields`], its values (one, two or three numbers; a switch is 0 or 1).
    pub fn form_values(&self, layer: usize) -> Vec<(FormField, [f32; 3])> {
        self.form_at(layer).map_or_else(Vec::new, |at| self.shape_values(at))
    }

    /// The numbers of the shape whose values are at a place.
    pub fn shape_values(&self, at: usize) -> Vec<(FormField, [f32; 3])> {
        let Some(kind) = self.shape_kind(at) else { return Vec::new() };
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

    /// The places of the initializers and of the updaters of a set.
    fn group_rules(&self, group: usize) -> (Vec<usize>, Vec<usize>) {
        let set = self.sets[group];
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

    /// How the game draws the particles of a layer.
    /// How the particles of a set are drawn.
    pub fn group_builder(&self, group: usize) -> Builder {
        match rel_at(&self.data, self.sets[group] + SET_BUILDER).map(|at| self.u32_at(at)) {
            Some(0x4000_0000) => Builder::WorldBillboard,
            Some(0x2000_0000) => Builder::UpBillboard,
            Some(0x1000_0000) => Builder::Plane,
            _ => Builder::Billboard,
        }
    }

    /// The place of the particle in its picture: the picture moves by this much, in halves of its sides.
    pub fn group_offset(&self, group: usize) -> [f32; 3] {
        rel_at(&self.data, self.sets[group] + SET_BUILDER).map_or([0.0; 3], |at| self.vector_at(at + 8))
    }

    /// True when the newest particles of a set are drawn over the older ones.
    pub fn group_oldest_first(&self, group: usize) -> bool {
        rel_at(&self.data, self.sets[group] + SET_BUILDER).is_none_or(|at| self.data.get(at + 4).is_none_or(|&b| b != 0))
    }

    /// The order of a set among the others when they are drawn: its priority, then its place in the model.
    pub fn group_order(&self, group: usize) -> (u32, u32) {
        (self.u32_at(self.sets[group] + SET_PRIORITY), self.u32_at(self.sets[group] + SET_SHAPE_INDEX))
    }

    /// True when a particle that another one makes starts with the speed of that one too.
    pub fn group_inherits_speed(&self, group: usize) -> bool {
        self.data.get(self.sets[group] + SET_INHERIT_SPEED).is_some_and(|&b| b != 0)
    }

    /// The place of the material of a set: the one that has the name of the set.
    fn material_at(&self, group: usize) -> Option<usize> {
        let wanted = format!("{}Material", self.group_name(group));
        self.materials.iter().copied().find(|&m| m + MATERIAL_MIX + 4 <= self.data.len() && self.text_at(m + NAME) == wanted)
    }

    /// The material of a set of particles.
    pub fn group_material(&self, group: usize) -> Option<Material> {
        let at = self.material_at(group)?;
        let mix = rel_at(&self.data, at + MATERIAL_MIX).filter(|&mix| mix + MIX_BUFFER + 24 <= self.data.len())?;
        // The commands of the six steps and of the saved colour, as the graphics chip receives them.
        let words: Vec<u32> = (0..6).flat_map(|step| (0..6).map(move |k| mix + MIX_STEPS + 28 * step + 4 + 4 * k)).chain((0..6).map(|k| mix + MIX_BUFFER + 4 * k)).map(|place| self.u32_at(place)).collect();
        let (mut stages, buffer_color) = sango_core::model::combiner_of(&words);
        // The game gives each step one of the colours of the material, not the one of its commands.
        for (step, stage) in stages.iter_mut().enumerate() {
            let colour = self.u32_at(mix + MIX_STEPS + 28 * step) as usize;
            if colour < 6 {
                stage.constant = self.u32_at(at + MATERIAL_CONSTANTS + 4 * colour).to_le_bytes();
            }
        }
        let function = self.u32_at(at + MATERIAL_BLEND);
        let factor = |shift: u32| ((function >> shift) & 15) as u8;
        let test = self.u32_at(mix + MIX_ALPHA_TEST);
        let state = sango_core::model::RenderState {
            known: true,
            blend: true,
            color_source: factor(16),
            color_destination: factor(20),
            alpha_source: factor(24),
            alpha_destination: factor(28),
            alpha_test: (test & 1 != 0).then_some((((test >> 4) & 7) as u8, ((test >> 8) & 0xff) as u8)),
            depth_write: false,
            ..Default::default()
        };
        // The matrix of the coordinates is stored after their numbers: three rows of four numbers.
        let matrix = at + MATERIAL_COORDINATES + 40;
        let row = |k: usize| [self.number_at(matrix + 16 * k), self.number_at(matrix + 16 * k + 4), self.number_at(matrix + 16 * k + 12)];
        let mapper = rel_at(&self.data, at + MATERIAL_MAPPERS);
        let texture = mapper.and_then(|mapper| rel_at(&self.data, mapper + 8));
        let setting = mapper.map(|mapper| self.u32_at(mapper + 36));
        // The mapper names the picture itself, or a texture that names it.
        let picture = texture.and_then(|object| {
            self.pictures.iter().position(|p| p.object == object).or_else(|| {
                let name = self.text_at(object + REFERENCE_NAME);
                self.pictures.iter().position(|p| p.name == name)
            })
        });
        Some(Material {
            stages,
            buffer_color,
            state,
            matrix: [row(0), row(1)],
            // The setting of the texture unit: the way of the axis S is in the bits 12 to 15, the one of T in the bits 8 to 11.
            wrap: setting.map_or([2, 2], |w| [(w >> 12) & 15, (w >> 8) & 15]),
            picture,
        })
    }

    /// The animation of a rule (the object that its first data points at), when it has one.
    fn animation_at(&self, rule: usize) -> Option<Animation> {
        let object = rel_at(&self.data, rule + 12)?;
        let length = self.u32_at(object + 8) as usize;
        let stride = self.u32_at(object + 16) as usize;
        let enabled_count = self.u32_at(object + 20) as usize;
        let enabled_at = rel_at(&self.data, object + 24)?;
        let data_count = self.u32_at(object + 28) as usize;
        let data_at = rel_at(&self.data, object + 32)?;
        if length == 0 || stride == 0 || enabled_count > 4 || data_count > 4096 || data_at + 4 * data_count > self.data.len() || enabled_at + enabled_count > self.data.len() {
            return None;
        }
        let (kind, duration) = match rel_at(&self.data, object + 4) {
            Some(option) => {
                let duration = self.number_at(option + 4);
                let kind = match self.u32_at(option) {
                    0x8000_0000 => AnimationKind::Fitting,
                    0x4000_0000 => AnimationKind::Loop { duration: self.u32_at(option + 8), random_offset: self.data.get(option + 12).is_some_and(|&b| b != 0) },
                    0x2000_0000 => AnimationKind::Random,
                    _ => AnimationKind::Frames,
                };
                (kind, duration)
            }
            None => (AnimationKind::Frames, 0.0),
        };
        Some(Animation { length, stride, enabled: self.data[enabled_at..enabled_at + enabled_count].iter().map(|&b| b != 0).collect(), data: (0..data_count).map(|k| self.f32_at(data_at + 4 * k)).collect(), kind, duration })
    }

    /// The constant values of the streams of a set: the stream and its numbers.
    pub fn group_parameters(&self, group: usize) -> Vec<(i32, Vec<f32>)> {
        let set = self.sets[group];
        let Some(collection) = rel_at(&self.data, set + COLLECTION) else { return Vec::new() };
        let count = self.u32_at(collection + 4) as usize;
        let Some(table) = rel_at(&self.data, collection + 8) else { return Vec::new() };
        if count > 32 {
            return Vec::new();
        }
        (0..count)
            .filter_map(|k| {
                let attribute = rel_at(&self.data, table + 4 * k)?;
                // A parameter has the type 0x40000000: the dimension, then the numbers.
                if self.u32_at(attribute) != 0x4000_0000 {
                    return None;
                }
                let (dimension, numbers) = (self.u32_at(attribute + 8) as usize, self.u32_at(attribute + 12) as usize);
                let at = rel_at(&self.data, attribute + 16)?;
                (dimension <= 4 && numbers <= 16 && at + 4 * numbers <= self.data.len()).then(|| (self.u32_at(attribute + 4) as i32, (0..numbers).map(|j| self.f32_at(at + 4 * j)).collect()))
            })
            .collect()
    }

    /// The place of the emitter of a layer, in the space of its particles.
    pub fn emitter_position(&self, layer: usize) -> [f32; 3] {
        self.vec3_at(self.emitters[layer] + EMITTER_POSITION)
    }

    /// The scale and the turn (around x, then y, then z, in radians) of the emitter of a layer.
    pub fn emitter_scale_and_turn(&self, layer: usize) -> ([f32; 3], [f32; 3]) {
        (self.vec3_at(self.emitters[layer] + EMITTER_SCALE), self.vec3_at(self.emitters[layer] + EMITTER_ROTATION))
    }

    /// A number of a rule, or 0 when the file stops before it.
    fn number_at(&self, at: usize) -> f32 {
        if at + 4 <= self.data.len() {
            self.f32_at(at)
        } else {
            0.0
        }
    }

    fn vector_at(&self, at: usize) -> [f32; 3] {
        [self.number_at(at), self.number_at(at + 4), self.number_at(at + 8)]
    }

    #[cfg(test)]
    /// The initializers of a layer, in the order of the file: the rules that give their first value to the streams of a new particle
    /// (the speed, the size, the turn, the colour, the transparency, the life). The types and the fields are the ones of the particle
    /// system of the game.
    pub fn initializers(&self, layer: usize) -> Vec<InitRule> {
        self.group_of(layer).map_or_else(Vec::new, |group| self.group_initializers(group))
    }

    /// The rules that give a new particle of a set its first values, in their order. The ones that are off are left out.
    pub fn group_initializers(&self, group: usize) -> Vec<InitRule> {
        self.group_rules(group)
            .0
            .into_iter()
            .filter(|&at| self.data[at + 5] != 0)
            .filter_map(|at| {
                let target = self.u32_at(at + OBJECT_TARGET) as i32;
                let (v, f) = (|k: usize| self.vector_at(at + 12 + 4 * k), |k: usize| self.number_at(at + 12 + 4 * k));
                let rule = match self.u32_at(at + OBJECT_TYPE) {
                    0x8000_0000 => Initializer::Directional { power: f(0), direction: v(1) },
                    0x4000_0000 => Initializer::RandomDirectional { power: f(0), direction: v(1), angle: f(4) },
                    0x2000_0000 => Initializer::Origin { power: f(0) },
                    0x1000_0000 => Initializer::Random { power: f(0) },
                    0x0800_0000 => Initializer::YAxis { power: f(0) },
                    0x0400_0000 => Initializer::Vector3 { value: v(0) },
                    0x0200_0000 => Initializer::Float { base: f(0), random: f(1) },
                    0x0100_0000 => Initializer::FloatRange { max: f(0), min: f(1) },
                    0x0080_0000 => Initializer::FloatImmediate { value: f(0) },
                    0x0040_0000 => Initializer::Vector2 { value: [f(0), f(1)] },
                    0x0020_0000 => Initializer::Vector3Random1 { base: v(0), random: f(3) },
                    0x0010_0000 => Initializer::Vector3Random3 { base: v(0), random: v(3) },
                    0x0008_0000 => Initializer::SpeedRandom { random: f(0) },
                    _ => return None,
                };
                Some(InitRule { target, rule })
            })
            .collect()
    }

    /// The rules that change the particles of a set at each frame, in their order. The ones that are off are left out.
    pub fn group_updaters(&self, group: usize) -> Vec<UpdRule> {
        self.group_rules(group)
            .1
            .into_iter()
            .filter(|&at| self.data[at + 5] != 0)
            .filter_map(|at| {
                let target = self.u32_at(at + OBJECT_TARGET) as i32;
                let kind = self.u32_at(at + OBJECT_TYPE);
                // The data of an updater starts after the reference to its animation (at 12).
                let (v, f) = (|k: usize| self.vector_at(at + 16 + 4 * k), |k: usize| self.number_at(at + 16 + 4 * k));
                let word = |k: usize| if at + 20 + 4 * k <= self.data.len() { self.u32_at(at + 16 + 4 * k) } else { 0 };
                let rule = match kind {
                    0x8000_0000 => Updater::Acceleration { factor: f(0) },
                    0x4000_0000 => Updater::General,
                    0x2000_0000 => Updater::Gravity { power: f(0), direction: v(1) },
                    0x1000_0000 => Updater::Spin { power: f(0), axis: v(1) },
                    0x0800_0000 => Updater::Random { power: v(0), interval: word(3) as i32 },
                    0x0002_0000 => Updater::Magnet { power: f(0), position: v(1), flags: word(4) },
                    0x0001_0000 => Updater::Collision { power: f(0), position: f(1), flags: word(2) },
                    0x0080_0002 => Updater::Add { value: v(0), animation: self.animation_at(at) },
                    0x0080_0004 => Updater::AddRandom { value: v(0) },
                    0x0080_0001 => Updater::Set { value: v(0), animation: self.animation_at(at) },
                    0x0040_0001 => Updater::SetNumber { value: f(0), animation: self.animation_at(at) },
                    0x0100_0000 => Updater::SetVector2 { value: [f(0), f(1)], animation: self.animation_at(at) },
                    0x0020_0000 => Updater::NumberKeys { in_time: word(0), out_time: word(1), values: [word(2) as i32, word(3) as i32, word(4) as i32, word(5) as i32, word(6) as i32] },
                    0x0010_0000 => Updater::VectorKeys { in_time: word(0), out_time: word(1), values: [v(2), v(5), v(8), v(11), v(14)] },
                    0x0004_0000 => Updater::TexturePattern { divisions: [word(0).max(1), word(1).max(1)], index: word(2) as i32, animation: self.animation_at(at) },
                    0x0008_0000 => Updater::TurnToMove { along_speed: word(0) == 0 },
                    0x0400_0000 => {
                        // The name of the set, the shape, the moment and the number.
                        let group = self.group_named(&self.text_at(at + 16));
                        let timing = rel_at(&self.data, at + 24).filter(|&t| t + 16 <= self.data.len()).map_or(ChildTiming::Birth, |t| match self.u32_at(t) {
                            0x4000_0000 => ChildTiming::Death,
                            0x2000_0000 => ChildTiming::LifeParts { start: self.f32_at(t + 4), end: self.f32_at(t + 8), interval: self.u32_at(t + 12) as i32 },
                            0x1000_0000 => ChildTiming::Frames { start: self.u32_at(t + 4) as i32, end: self.u32_at(t + 8) as i32, interval: self.u32_at(t + 12) as i32 },
                            _ => ChildTiming::Birth,
                        });
                        Updater::Child { group, shape: rel_at(&self.data, at + 20), timing, count: word(3) as i32 }
                    }
                    other => Updater::Other(other),
                };
                Some(UpdRule { target, rule })
            })
            .collect()
    }

    /// The name of a set of particles.
    pub fn group_name(&self, group: usize) -> String {
        self.text_at(self.sets[group] + SET_NAME)
    }

    /// The sets of the particles that the ones of a set make, and the ones that those make, each once.
    pub fn child_groups(&self, group: usize) -> Vec<usize> {
        let mut found = Vec::new();
        let mut to_see = vec![group];
        while let Some(from) = to_see.pop() {
            for rule in self.group_updaters(from) {
                if let Updater::Child { group: Some(child), .. } = rule.rule {
                    if child != group && !found.contains(&child) {
                        found.push(child);
                        to_see.push(child);
                    }
                }
            }
        }
        found.sort_unstable();
        found
    }

    /// The place of the numbers of the constant value of a stream of a set, and how many they are.
    fn parameter_at(&self, group: usize, target: u32) -> Option<(usize, usize)> {
        let collection = rel_at(&self.data, self.sets[group] + COLLECTION)?;
        let count = self.u32_at(collection + 4) as usize;
        let table = rel_at(&self.data, collection + 8).filter(|_| count <= 32)?;
        (0..count).filter_map(|k| rel_at(&self.data, table + 4 * k)).find_map(|attribute| {
            // A parameter has the type 0x40000000: the stream, the dimension, then the numbers.
            if self.u32_at(attribute) != 0x4000_0000 || self.u32_at(attribute + 4) != target {
                return None;
            }
            let numbers = self.u32_at(attribute + 12) as usize;
            let at = rel_at(&self.data, attribute + 16)?;
            (numbers >= 1 && numbers <= 16 && at + 4 * numbers <= self.data.len()).then_some((at, numbers))
        })
    }

    /// The first updater of a set that changes a stream, and true when it follows a curve of the file.
    fn updater(&self, group: usize, target: u32) -> Option<(usize, bool)> {
        let at = self.group_rules(group).1.into_iter().find(|&at| at + 84 <= self.data.len() && self.data[at + 5] != 0 && self.u32_at(at + OBJECT_TARGET) == target)?;
        let kind = self.u32_at(at + OBJECT_TYPE);
        let curve = matches!(kind, FIXED_VECTOR | FIXED_NUMBER) && self.animation_at(at).is_some();
        Some((at, curve))
    }

    /// The first initializer of a kind that gives a stream its first value.
    fn initializer(&self, group: usize, target: u32, kind: u32) -> Option<usize> {
        self.group_rules(group).0.into_iter().find(|&at| at + 28 <= self.data.len() && self.data[at + 5] != 0 && self.u32_at(at + OBJECT_TARGET) == target && self.u32_at(at + OBJECT_TYPE) == kind)
    }

    /// The place of the three numbers that give a stream of a set when no rule changes it: the ones of an initializer, else
    /// the constant value of the set.
    fn constant_vector(&self, group: usize, target: u32, kind: u32) -> Option<usize> {
        self.initializer(group, target, kind).map(|at| at + 12).or_else(|| {
            let (at, numbers) = self.parameter_at(group, target)?;
            (numbers >= 3).then_some(at)
        })
    }

    /// A stream of three numbers during the life of the particles: the curve of an updater, else the constant value.
    fn vector_curve(&self, group: usize, target: u32, kind: u32) -> Option<ColorCurve> {
        let fixed = |value: [f32; 3]| ColorCurve { fixed: true, start: value, middle: value, end: value, in_time: 0.0, out_time: 1.0 };
        if let Some((at, curve)) = self.updater(group, target) {
            return match self.u32_at(at + OBJECT_TYPE) {
                FIXED_VECTOR if !curve => Some(fixed(self.vec3_at(at + 16))),
                KEYED_VECTOR => {
                    let (slope, offset) = (self.vec3_at(at + 60), self.vec3_at(at + 72));
                    Some(ColorCurve { fixed: false, start: self.vec3_at(at + 24), middle: self.vec3_at(at + 36), end: [offset[0] + slope[0], offset[1] + slope[1], offset[2] + slope[2]], in_time: self.u32_at(at + 16) as f32 / TIME_SCALE, out_time: self.u32_at(at + 20) as f32 / TIME_SCALE })
                }
                _ => None,
            };
        }
        self.constant_vector(group, target, kind).map(|at| fixed(self.vec3_at(at)))
    }

    fn set_vector_curve(&mut self, group: usize, target: u32, kind: u32, curve: &ColorCurve) {
        if let Some((at, animated)) = self.updater(group, target) {
            match self.u32_at(at + OBJECT_TYPE) {
                FIXED_VECTOR if !animated => self.put_vec3(at + 16, curve.middle),
                KEYED_VECTOR => self.write_vector_curve(at, curve.start, curve.middle, curve.end, curve.in_time, curve.out_time),
                _ => {}
            }
        } else if let Some(at) = self.constant_vector(group, target, kind) {
            self.put_vec3(at, curve.middle);
        }
    }

    /// The colour of the particles of a set. `None` when it follows a curve of the file.
    pub fn color(&self, group: usize) -> Option<ColorCurve> {
        self.vector_curve(group, TARGET_COLOR, COLOR_INITIALIZER)
    }

    /// Writes the colour of the particles of a set.
    pub fn set_color(&mut self, group: usize, color: &ColorCurve) {
        self.set_vector_curve(group, TARGET_COLOR, COLOR_INITIALIZER, color);
    }

    /// The colours of the material of a set that its picture is mixed with: the number of each one and its red, green and
    /// blue.
    pub fn material_colors(&self, group: usize) -> Vec<(usize, [f32; 3])> {
        let (Some(material), Some(at)) = (self.group_material(group), self.material_at(group)) else { return Vec::new() };
        let Some(mix) = rel_at(&self.data, at + MATERIAL_MIX) else { return Vec::new() };
        let mut found: Vec<(usize, [f32; 3])> = Vec::new();
        for (step, stage) in material.stages.iter().enumerate() {
            // The number of values that the way of mixing takes.
            let count = match stage.color_mode {
                0 => 1,
                4 | 8 | 9 => 3,
                _ => 2,
            };
            let colour = self.u32_at(mix + MIX_STEPS + 28 * step) as usize;
            // A value that takes the colour itself, not its opacity alone.
            let reads = (0..count).any(|k| stage.color_source[k] == 14 && !matches!(stage.color_operand[k], 2 | 3));
            if reads && colour < 6 && !found.iter().any(|&(c, _)| c == colour) {
                found.push((colour, [0, 1, 2].map(|k| f32::from(stage.constant[k]) / 255.0)));
            }
        }
        found
    }

    /// Writes one colour of the material of a set (see `material_colors`). Its opacity stays.
    pub fn set_material_color(&mut self, group: usize, colour: usize, value: [f32; 3]) {
        let Some(at) = self.material_at(group).filter(|_| colour < 6) else { return };
        for (k, part) in value.into_iter().enumerate() {
            let part = part.clamp(0.0, 1.0);
            // The colour is kept twice: as bytes, which the game reads, and as numbers.
            self.data[at + MATERIAL_CONSTANTS + 4 * colour + k] = (part * 255.0).round() as u8;
            self.put_f32(at + MATERIAL_COLOR_NUMBERS + 16 * colour + 4 * k, part);
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

    /// The place of the number that gives the opacity of a set when no rule changes it.
    fn constant_alpha(&self, group: usize) -> Option<usize> {
        self.initializer(group, TARGET_ALPHA, OPACITY_INITIALIZER).map(|at| at + 12).or_else(|| self.parameter_at(group, TARGET_ALPHA).map(|(at, _)| at))
    }

    /// The transparency of the particles of a set (0 is clear, 1 is solid). `None` when it follows a curve of the file.
    pub fn alpha(&self, group: usize) -> Option<NumberCurve> {
        let constant = |value: f32| NumberCurve { fixed: true, start: value, peak: value, end: value, in_time: 0.0, out_time: 1.0 };
        let fixed = |raw: i32| raw as f32 / TIME_SCALE;
        let Some((at, curve)) = self.updater(group, TARGET_ALPHA) else { return self.constant_alpha(group).map(|at| constant(self.f32_at(at))) };
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_NUMBER if !curve => Some(constant(self.f32_at(at + 16))),
            KEYED_NUMBER => {
                let raw = |k: usize| self.u32_at(at + 24 + 4 * k) as i32;
                Some(NumberCurve { fixed: false, start: fixed(raw(0)), peak: fixed(raw(1)), end: fixed(raw(4)) + fixed(raw(3)), in_time: self.u32_at(at + 16) as f32 / TIME_SCALE, out_time: self.u32_at(at + 20) as f32 / TIME_SCALE })
            }
            _ => None,
        }
    }

    pub fn set_alpha(&mut self, group: usize, curve: &NumberCurve) {
        let Some((at, animated)) = self.updater(group, TARGET_ALPHA) else {
            if let Some(at) = self.constant_alpha(group) {
                self.put_f32(at, curve.peak);
            }
            return;
        };
        match self.u32_at(at + OBJECT_TYPE) {
            FIXED_NUMBER if !animated => self.put_f32(at + 16, curve.peak),
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

    /// The stream that gives the size of the particles: the extra size when a rule of the set gives it, else the size.
    fn size_target(&self, group: usize) -> u32 {
        let given = |target: u32| self.updater(group, target).is_some() || self.initializer(group, target, SIZE_INITIALIZER).is_some();
        if given(TARGET_SIZE_EXTRA) {
            TARGET_SIZE_EXTRA
        } else if given(TARGET_SIZE) || self.parameter_at(group, TARGET_SIZE).is_some() {
            TARGET_SIZE
        } else {
            TARGET_SIZE_EXTRA
        }
    }

    /// The size of the particles of a set, at the start, in the middle and at the end of their life. `None` when it follows
    /// a curve of the file.
    pub fn size(&self, group: usize) -> Option<ColorCurve> {
        self.vector_curve(group, self.size_target(group), SIZE_INITIALIZER)
    }

    pub fn set_size(&mut self, group: usize, size: &ColorCurve) {
        self.set_vector_curve(group, self.size_target(group), SIZE_INITIALIZER, size);
    }

    /// The life of the particles of a set, in frames: the shortest and the longest. The two are one number when
    /// `life_is_one_number` is true.
    pub fn life(&self, group: usize) -> Option<(f32, f32)> {
        if let Some(at) = self.initializer(group, TARGET_LIFE, LIFE_INITIALIZER) {
            return Some((self.f32_at(at + 16), self.f32_at(at + 12)));
        }
        let (at, _) = self.parameter_at(group, TARGET_LIFE)?;
        let value = self.f32_at(at);
        Some((value, value))
    }

    /// True when all the particles of a set live the same number of frames, given by the set.
    pub fn life_is_one_number(&self, group: usize) -> bool {
        self.initializer(group, TARGET_LIFE, LIFE_INITIALIZER).is_none()
    }

    pub fn set_life(&mut self, group: usize, shortest: f32, longest: f32) {
        let (shortest, longest) = (shortest.max(0.0), longest.max(shortest.max(0.0)));
        if let Some(at) = self.initializer(group, TARGET_LIFE, LIFE_INITIALIZER) {
            self.put_f32(at + 12, longest);
            self.put_f32(at + 16, shortest);
        } else if let Some((at, _)) = self.parameter_at(group, TARGET_LIFE) {
            self.put_f32(at, longest);
        }
    }

    /// The most particles that a set can have alive at once.
    pub fn capacity(&self, group: usize) -> Option<i32> {
        let collection = rel_at(&self.data, self.sets[group] + COLLECTION)?;
        Some(self.u32_at(collection) as i32)
    }

    pub fn set_capacity(&mut self, group: usize, capacity: i32) {
        if let Some(collection) = rel_at(&self.data, self.sets[group] + COLLECTION) {
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
        let group = effect.group_of(0).unwrap();
        if let Some(mut color) = effect.color(group) {
            color.start = [1.0, 0.0, 0.0];
            color.middle = [0.0, 1.0, 0.0];
            color.end = [0.0, 0.0, 1.0];
            effect.set_color(group, &color);
            let again = effect.color(group).unwrap();
            if !color.fixed {
                assert!((again.start[0] - 1.0).abs() < 1e-4 && (again.end[2] - 1.0).abs() < 1e-3, "{again:?}");
            }
        }
        assert_eq!(effect.bytes().len(), bytes.len(), "the size of the file does not change");
        assert_eq!(Effect::parse(effect.bytes()).unwrap().emission(0), emission);
    }

    /// With the dump of the game: every set of particles gives its colour, opacity, size and life, unless a curve of the
    /// file gives them, and a new value is read back.
    #[test]
    fn the_values_of_every_set_are_changed_and_read_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(31)).unwrap();
        // The sets, then how many give each value.
        let (mut sets, mut found) = (0usize, [0usize; 4]);
        let mut colours = 0usize;
        for id in 0..garc.file_count() {
            let Some(Ok(mut effect)) = garc.first_file_decompressed(id).map(|bytes| Effect::parse(&bytes)) else { continue };
            for group in 0..effect.group_count() {
                sets += 1;
                if let Some(mut curve) = effect.color(group) {
                    found[0] += 1;
                    (curve.start, curve.middle, curve.end) = ([0.25, 0.5, 0.75], [0.25, 0.5, 0.75], [0.25, 0.5, 0.75]);
                    effect.set_color(group, &curve);
                    let again = effect.color(group).unwrap();
                    assert!((0..3).all(|k| (again.middle[k] - curve.middle[k]).abs() < 1e-3), "colour of the set {group} of {id}: {again:?}");
                }
                if let Some(mut curve) = effect.alpha(group) {
                    found[1] += 1;
                    (curve.start, curve.peak, curve.end) = (0.5, 0.5, 0.5);
                    effect.set_alpha(group, &curve);
                    assert!((effect.alpha(group).unwrap().peak - 0.5).abs() < 1e-3, "opacity of the set {group} of {id}");
                }
                if let Some(mut curve) = effect.size(group) {
                    found[2] += 1;
                    (curve.start, curve.middle, curve.end) = ([3.0; 3], [3.0; 3], [3.0; 3]);
                    effect.set_size(group, &curve);
                    assert!((effect.size(group).unwrap().middle[0] - 3.0).abs() < 1e-3, "size of the set {group} of {id}");
                }
                if effect.life(group).is_some() {
                    found[3] += 1;
                    effect.set_life(group, 12.0, 12.0);
                    assert_eq!(effect.life(group), Some((12.0, 12.0)), "life of the set {group} of {id}");
                }
                for (colour, _) in effect.material_colors(group) {
                    colours += 1;
                    effect.set_material_color(group, colour, [1.0, 0.0, 0.2]);
                    let again = effect.material_colors(group).into_iter().find(|c| c.0 == colour).unwrap().1;
                    assert_eq!(again.map(|v| (v * 255.0).round() as u8), [255, 0, 51]);
                }
            }
        }
        assert!(found[3] == sets && found.iter().all(|&n| n * 10 > sets * 9), "{sets} sets, {found:?}");
        assert!(colours > sets / 3);
    }
}
