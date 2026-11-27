//! A choreography played frame after frame, as the battle plays it: the steps start in the order of the file, each one
//! moves a Pokémon, the camera, an effect or a model at once or over its frames, and the objects follow what they were
//! told to follow. The state at any frame comes from the frames before it.
//!
//! The order of one frame is the one of the game: the motions move on, the steps of the frame start, the movements over
//! several frames advance, then the objects follow their targets.

use crate::assets::{attack_motion, Library, Pokemon};
use crate::debris::{self, Debris};
use crate::math::{self, V3};
use sango_choreography::{kind_of, MoveChoreography, Step, Value, FILE_OF_NO_MOVE};
use sango_core::animation::Look;
use sango_core::motion::{MotionPack, Pose};
use sango_core::posing::{bone_of_joints, posed_worlds, rest_worlds, Affine};
use sango_core::SdCard;
use std::collections::{BTreeMap, HashMap};
use std::f32::consts::PI;
use std::rc::Rc;

/// The battle motions that the steps name.
pub mod motions {
    pub const WAIT: usize = 0;
    pub const LAND_A: usize = 3;
    pub const LAND_B: usize = 4;
    pub const PHYSICAL_1: usize = 8;
    pub const SPECIAL_4: usize = 15;
    pub const FAINT: usize = 17;
}

/// The motion of each pose of the steps "Pokemon Play Pose", from the pose 0 to the pose 22.
const POSES: [usize; 23] = [0, 8, 12, 7, 16, 3, 16, 16, 17, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 1, 2];
/// The first pose that asks the table of the attacks for a motion.
const FIRST_ATTACK_POSE: i32 = 30;

/// A hit Pokémon goes this far back (for a Pokémon of one meter), in this many frames, then returns.
const KNOCKBACK: f32 = 60.0;
const KNOCKBACK_AWAY: u32 = 3;
const KNOCKBACK_RETURN: u32 = 11;
/// The frames of a hit that stays until the camera starts again.
const KNOCKBACK_HELD: i32 = 6;

/// The frames of a screen fade that gives no length.
const DEFAULT_FADE: u32 = 8;

/// The camera of a battle: how wide it sees and what it does not see.
pub const CAMERA_ANGLE: f32 = 30.0;
pub const CAMERA_NEAR: f32 = 16.0;

/// How the battle is set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Setup {
    /// True when the attacker is the Pokémon of the opponent (far from the camera).
    pub attacker_is_opponent: bool,
    /// True when the move has the attacker as its target.
    pub targets_user: bool,
}

/// The attacker is the first Pokémon of the cast, the target the second one.
pub const ATTACKER: usize = 0;
pub const TARGET: usize = 1;

/// An object that the steps place: where it is, how it is turned and sized, and what it follows.
#[derive(Clone, Debug)]
pub struct Object {
    pub position: V3,
    /// A move on top of the position (the shakes).
    pub offset: V3,
    /// The turns around x, y and z, in radians.
    pub rotation: V3,
    pub scale: V3,
    /// A size that comes from the Pokémon the object is placed on.
    pub adjust: V3,
    /// True when the object is shown the other way round (the attacker is the opponent).
    pub flip: bool,
    /// True when the object turns to the way that it moves.
    pub auto_rotate: bool,
    before: V3,
    follow: Follow,
    follow_place: V3,
    follow_turn: V3,
    follow_axes: i32,
}

impl Default for Object {
    fn default() -> Self {
        Object { position: [0.0; 3], offset: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3], adjust: [1.0; 3], flip: false, auto_rotate: false, before: [0.0; 3], follow: Follow::None, follow_place: [0.0; 3], follow_turn: [0.0; 3], follow_axes: 0 }
    }
}

impl Object {
    /// The place, the turns and the size that are drawn.
    pub fn drawn(&self) -> (V3, V3, V3) {
        let mut turn = self.rotation;
        let mut size = math::mul(self.scale, self.adjust);
        if self.flip {
            turn[0] = -turn[0];
            size[2] = -size[2];
        }
        (math::add(self.position, self.offset), turn, size)
    }

    pub fn matrix(&self) -> Affine {
        let (place, turn, size) = self.drawn();
        math::matrix(place, turn, size)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Follow {
    None,
    /// A point of a Pokémon.
    Pokemon { who: usize, point: i32 },
    /// A bone of a model.
    Bone { group: u16, name: String },
    /// The place of a model.
    Model { group: u16 },
}

/// A motion that plays on a Pokémon or on a model.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub index: usize,
    pub frame: f32,
    pub speed: f32,
    pub repeat: bool,
}

#[derive(Clone, Debug)]
pub struct PokemonState {
    pub object: Object,
    pub visible: bool,
    /// False while a move hides the Pokémon (it digs or flies).
    pub charging_visible: bool,
    pub shadow: bool,
    pub motion: Motion,
    /// The motion that follows when the one that plays ends.
    reserve: Option<usize>,
    /// The frames since the start: the endless animation follows them.
    pub clock: f32,
    /// True when the Pokémon stays pushed back until the camera starts again.
    pushed: bool,
    pub tint: V3,
    pub tint_strength: f32,
}

#[derive(Clone, Debug)]
pub struct EffectState {
    pub serial: u32,
    pub number: i32,
    pub object: Object,
    pub born: i32,
    /// The frames that the effect stays, when its step has a length.
    pub life: Option<i32>,
    /// The frame from which it makes no new particle.
    pub stopped: Option<i32>,
    /// True when it is drawn before the Pokémon.
    pub behind: bool,
    /// True when it stays in front of the camera.
    pub on_screen: bool,
}

/// The place of a model in the order of the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    BeforePokemon,
    AfterPokemon,
    WithEffects,
}

/// An animation of the materials or of the visibility of a model.
#[derive(Clone, Copy, Debug)]
pub struct Animation {
    pub file: i32,
    pub frame: f32,
    pub repeat: bool,
}

#[derive(Clone, Debug)]
pub struct ModelState {
    pub serial: u32,
    pub number: i32,
    pub object: Object,
    pub layer: Layer,
    pub visible: bool,
    /// The file of the motions and the motion that plays.
    pub motion: Option<(i32, Motion)>,
    pub animations: Vec<Animation>,
    pub speed: f32,
    /// The colours of the materials that the steps set: the material, the colour and its value.
    pub colours: Vec<(String, usize, [f32; 4])>,
    pub on_screen: bool,
}

/// A set of debris on the stage.
#[derive(Clone, Debug)]
pub struct DebrisState {
    pub serial: u32,
    pub number: i32,
    pub object: Object,
    pub layer: Layer,
    pub animations: Vec<(i32, bool)>,
    pub set: Debris,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: V3,
    pub target: V3,
    pub offset: V3,
    /// The angle that the camera sees from the top to the bottom, in degrees.
    pub angle: f32,
    pub up: V3,
}

/// A camera animation that plays: the file, its frame, and how it is placed on the stage.
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub file: i32,
    pub frame: f32,
    pub offset: V3,
    /// The turn around the vertical axis, in radians.
    pub turn: f32,
    pub scale: V3,
    pub mirrored: bool,
    pub up_mirrored: bool,
}

impl Clip {
    fn new(file: i32) -> Clip {
        Clip { file, frame: 0.0, offset: [0.0; 3], turn: 0.0, scale: [1.0; 3], mirrored: false, up_mirrored: false }
    }

    /// A size below 0 along x mirrors the animation.
    fn set_scale(&mut self, scale: V3) {
        let mut scale = scale;
        if scale[0] < 0.0 {
            scale[0] = -scale[0];
            self.mirrored = true;
        }
        self.scale = scale;
    }

    /// A point of the animation on the stage.
    fn place(&self, point: V3) -> V3 {
        let mut point = math::mul(point, self.scale);
        math::turn_xz(&mut point, self.turn);
        if self.mirrored {
            point[0] = -point[0];
        }
        math::add(point, self.offset)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Screen {
    /// The colour that covers the place of the battle behind the Pokémon, and whether it is on.
    pub fill: [f32; 4],
    pub fill_on: bool,
    pub place_shown: bool,
    pub plates_shown: bool,
    /// The colour that covers the whole screen.
    pub fade: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ref {
    Pokemon(usize),
    Effect(u16, u32),
    Model(u16, u32),
    Debris(u16, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Property {
    Position,
    Offset,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Curve {
    Linear,
    FastStart,
    SlowStart,
    EaseBack,
}

impl Curve {
    fn of(number: i32) -> Curve {
        match number {
            1 => Curve::FastStart,
            2 => Curve::SlowStart,
            _ => Curve::Linear,
        }
    }
}

/// A change from a value to another over some frames.
#[derive(Clone, Copy, Debug)]
struct Tween<const N: usize> {
    start: [f32; N],
    end: [f32; N],
    steps: u32,
    step: u32,
    curve: Curve,
}

impl<const N: usize> Tween<N> {
    fn new(start: [f32; N], end: [f32; N], steps: u32, curve: Curve) -> Self {
        Tween { start, end, steps, step: 0, curve }
    }

    /// The value one frame later, and true when the change is over.
    fn advance(&mut self) -> ([f32; N], bool) {
        self.step = self.step.saturating_add(1);
        if self.step >= self.steps {
            return (self.end, true);
        }
        let part = self.step as f32 / self.steps as f32;
        let value = std::array::from_fn(|k| {
            let (a, b) = (self.start[k], self.end[k]);
            match self.curve {
                Curve::Linear => a + (b - a) * part,
                Curve::FastStart => a + (b - a) * (1.0 - (1.0 - part) * (1.0 - part)).max(0.0).sqrt(),
                Curve::SlowStart => b - (b - a) * (1.0 - part * part).max(0.0).sqrt(),
                // The turn of the camera with this curve goes from the end back to the start, as in the game.
                Curve::EaseBack => b - (b - a) * ((PI + part * PI).cos() + 1.0) / 2.0,
            }
        });
        (value, false)
    }
}

#[derive(Clone, Debug)]
enum Task {
    Change { target: Ref, property: Property, tween: Tween<3> },
    Shake { target: Ref, axis: usize, angle: Tween<1>, amount: Tween<1> },
    FollowCamera { target: Ref, distance: f32, turn: bool, left: i32 },
    Arc { target: Ref, speed: f32, gravity: f32, height: f32, end: f32 },
    CameraMove { position: Tween<3>, target: Tween<3>, angle: Tween<1> },
    CameraShake { axis: usize, angle: Tween<1>, amount: Tween<1> },
    CameraTurn { target: V3, height: f32, radius: f32, angle: Tween<1> },
    PokemonTint { who: usize, color: Tween<3>, strength: Tween<1> },
    Fill { color: Tween<4> },
    Colour { target: Ref, material: String, index: usize, color: Tween<4> },
    Delete { target: Ref, left: i32 },
    ClipFollow { who: usize, point: i32, offset: V3, option: i32, left: i32 },
    /// A Pokémon that is hit goes back, then returns.
    Knockback { who: usize, tween: Tween<3>, returning: bool },
    Fade { color: Tween<4> },
}

/// Everything that is on the stage at a frame.
#[derive(Clone, Debug)]
pub struct State {
    pub frame: i32,
    pub pokemon: [PokemonState; 2],
    pub effects: BTreeMap<u16, EffectState>,
    pub models: BTreeMap<u16, ModelState>,
    pub debris: BTreeMap<u16, DebrisState>,
    pub camera: Camera,
    /// The camera animation that moves the camera.
    pub clip: Option<Clip>,
    pub screen: Screen,
    tasks: Vec<Task>,
    next: usize,
    /// The step where the game stops reading the file: it starts before the step above it.
    pub blocked: Option<usize>,
    attack_started: bool,
    serial: u32,
}

/// The steps that the viewer does not show, by name.
pub fn is_shown(name: &str) -> bool {
    let hidden = ["Sound", "Health Bar", "Break Objects", "Trainer", "Overlay", "Message", "Chain Attack", "Mega Evolution", "Aegislash", "Weather", "Field Effect", "Input Window", "Debug", "Substitute"];
    let single = ["Camera Screen Depth Effect", "Camera Flat View", "Camera Clip On Pokemon Quad", "Wait For Animation", "Pokemon Outline", "Pokemon Gaze At", "Pokemon Shade Effect", "Pokemon Point Light", "Model Spawn Ball", "Model Light Direction", "Model Ball Light Direction", "Model Replace Texture", "Pokemon Sky Pose", "Pokemon Happy Pose", "Pokemon Wild Cry"];
    !(hidden.iter().any(|family| name.starts_with(family)) || single.contains(&name) || name.ends_with(" Quad"))
}

/// True for a step that only marks a part of the file: it does nothing in the game either.
pub fn is_mark(name: &str) -> bool {
    name.starts_with("Section ") || name == "Placeholder" || name == "Note"
}

/// A choreography with its Pokémon, ready to be played.
pub struct Run {
    pub choreography: MoveChoreography,
    pub setup: Setup,
    pub cast: [Rc<Pokemon>; 2],
    /// The motion of the attack of the attacker and the frame where it starts.
    attack: Option<(usize, i32)>,
    /// The first frame: below 0 when the attack must start before the first step.
    pub first: i32,
    /// The last frame.
    pub last: i32,
    pub state: State,
    nodes: HashMap<(usize, usize, u32), Rc<Vec<Affine>>>,
}

/// The kinds of the attacks that start by themselves with the step "Pokemon Play Attack Pose".
fn attack_kind(pose: i32) -> Option<usize> {
    match pose {
        1 => Some(0),
        2 => Some(1),
        _ => usize::try_from(pose - FIRST_ATTACK_POSE).ok(),
    }
}

fn int(step: &Step, name: &str) -> i32 {
    match value(step, name) {
        Some(Value::Int(v)) => v,
        Some(Value::Bool(v)) => i32::from(v),
        _ => 0,
    }
}

fn flag(step: &Step, name: &str) -> bool {
    int(step, name) != 0
}

fn float(step: &Step, name: &str) -> f32 {
    match value(step, name) {
        Some(Value::Float(v)) if v.is_finite() => v,
        _ => 0.0,
    }
}

fn vector(step: &Step, name: &str) -> V3 {
    match value(step, name) {
        Some(Value::Vec3(v)) => v.map(|c| if c.is_finite() { c } else { 0.0 }),
        _ => [0.0; 3],
    }
}

fn text(step: &Step, name: &str) -> String {
    match value(step, name) {
        Some(Value::Text(v)) => v,
        _ => String::new(),
    }
}

fn value(step: &Step, name: &str) -> Option<Value> {
    let param = kind_of(step.kind)?.params.iter().find(|p| p.name == name)?;
    (param.offset + param.size <= step.data.len()).then(|| step.value(param))
}

/// A size of 0 would flatten an object: the game takes a very small one.
fn never_flat(size: V3) -> V3 {
    size.map(|v| if v == 0.0 { 0.01 } else { v })
}

/// The point of view of a battle at rest, from the sizes of the two Pokémon (the one near the camera, then the far one).
pub fn default_camera(near: usize, far: usize) -> Camera {
    const PLACES: [[V3; 3]; 3] = [[[125.0, 89.0, 342.0], [155.0, 79.0, 386.0], [186.0, 77.0, 410.0]], [[233.0, 138.0, 434.0], [238.0, 136.0, 453.0], [243.0, 134.0, 472.0]], [[170.0, 200.0, 419.0], [191.0, 199.0, 466.0], [218.0, 198.0, 513.0]]];
    const TARGETS: [[V3; 3]; 3] = [[[-50.0, 15.0, 0.0], [-47.0, 42.0, 0.0], [-35.0, 70.0, 0.0]], [[-50.0, 60.0, 0.0], [-35.0, 70.0, 0.0], [-20.0, 80.0, 0.0]], [[-20.0, 120.0, 88.0], [-10.0, 125.0, 88.0], [0.0, 130.0, 88.0]]];
    let (near, far) = (near.min(2), far.min(2));
    Camera { position: PLACES[near][far], target: TARGETS[near][far], offset: [0.0; 3], angle: CAMERA_ANGLE, up: [0.0, 1.0, 0.0] }
}

impl Run {
    /// Prepares a choreography: the file number says the move, which says the motion of the attack.
    pub fn new(library: &mut Library, file: usize, choreography: MoveChoreography, setup: Setup, cast: [Rc<Pokemon>; 2]) -> Run {
        let attacker = cast[ATTACKER].clone();
        let move_id = file.saturating_sub(FILE_OF_NO_MOVE);
        let table = library.attack_table();
        // The last step of the file that plays an attack decides: the motion starts so that its key frame is on the
        // first frame of the step.
        let attack = choreography
            .steps
            .iter()
            .filter(|step| kind_of(step.kind).is_some_and(|k| k.name == "Pokemon Play Attack Pose"))
            .filter_map(|step| {
                let kind = attack_kind(int(step, "pose"))?;
                let found = attack_motion(&table, attacker.choice.species, attacker.choice.form, move_id, kind);
                let motion = found.map_or(motions::WAIT, |m| motions::PHYSICAL_1 + m);
                let key = attacker.keys.get(motion).copied().flatten().unwrap_or(20);
                Some((motion, step.start - key))
            })
            .next_back();
        let first = attack.map_or(0, |(_, start)| start.min(0));
        let last = choreography.length().max(choreography.last_frame()).max(1);
        let state = Self::start(&cast, setup, first);
        Run { choreography, setup, cast, attack, first, last, state, nodes: HashMap::new() }
    }

    /// The stage before the first frame.
    fn start(cast: &[Rc<Pokemon>; 2], setup: Setup, first: i32) -> State {
        let pokemon = std::array::from_fn(|who| {
            let (place, turn) = Self::home_of(cast, setup, who);
            PokemonState {
                object: Object { position: place, rotation: [0.0, turn.to_radians(), 0.0], ..Default::default() },
                visible: true,
                charging_visible: true,
                shadow: true,
                motion: Motion { index: motions::WAIT, frame: 0.0, speed: 1.0, repeat: true },
                reserve: None,
                clock: 0.0,
                pushed: false,
                tint: [0.0; 3],
                tint_strength: 0.0,
            }
        });
        let (near, far) = if setup.attacker_is_opponent { (TARGET, ATTACKER) } else { (ATTACKER, TARGET) };
        State {
            frame: first - 1,
            pokemon,
            effects: BTreeMap::new(),
            models: BTreeMap::new(),
            debris: BTreeMap::new(),
            camera: default_camera(cast[near].size, cast[far].size),
            clip: None,
            screen: Screen { fill: [0.0; 4], fill_on: false, place_shown: true, plates_shown: true, fade: [0.0; 4] },
            tasks: Vec::new(),
            next: 0,
            blocked: None,
            attack_started: false,
            serial: 0,
        }
    }

    /// True when a Pokémon of the cast is on the side of the player (near the camera).
    pub fn is_near(setup: Setup, who: usize) -> bool {
        (who == ATTACKER) != setup.attacker_is_opponent
    }

    /// The place of a Pokémon at rest, and its turn around the vertical axis in degrees.
    fn home_of(cast: &[Rc<Pokemon>; 2], setup: Setup, who: usize) -> (V3, f32) {
        let distance = [200.0, 220.0, 250.0][cast[who].size.min(2)];
        if Self::is_near(setup, who) {
            ([0.0, 0.0, distance], 180.0)
        } else {
            ([0.0, 0.0, -distance], 0.0)
        }
    }

    fn home(&self, who: usize) -> (V3, f32) {
        Self::home_of(&self.cast, self.setup, who)
    }

    /// True when the pictures and the movements are turned the other way: the opponent attacks.
    fn reversed(&self) -> bool {
        self.setup.attacker_is_opponent
    }

    /// The Pokémon of a target of a step.
    fn who(&self, target: i32) -> usize {
        match target {
            1 | 2 if !self.setup.targets_user => TARGET,
            _ => ATTACKER,
        }
    }

    /// The Pokémon that a move aims at.
    fn aimed(&self) -> usize {
        if self.setup.targets_user {
            ATTACKER
        } else {
            TARGET
        }
    }

    /// Shows the stage at a frame. A frame before the one that is shown plays the file again from the start.
    pub fn seek(&mut self, library: &mut Library, sd: Option<&SdCard>, frame: i32) {
        let frame = frame.clamp(self.first, self.last);
        if frame < self.state.frame {
            self.state = Self::start(&self.cast, self.setup, self.first);
        }
        while self.state.frame < frame {
            self.advance(library, sd);
        }
    }

    fn advance(&mut self, library: &mut Library, sd: Option<&SdCard>) {
        self.state.frame += 1;
        let frame = self.state.frame;
        if frame > self.first {
            self.move_motions(library);
        }
        if let Some((motion, start)) = self.attack {
            if !self.state.attack_started && start <= frame && !(start == 1 && frame == 0) {
                self.state.attack_started = true;
                self.play(ATTACKER, motion, false, Some(motions::WAIT));
            }
        }
        if frame >= 0 {
            // The steps of this frame, in the order of the file. A step that starts before the frame stops the reading.
            while let Some(step) = self.choreography.steps.get(self.state.next) {
                if step.start != frame {
                    if step.start < frame {
                        self.state.blocked = Some(self.state.next);
                    }
                    break;
                }
                let step = step.clone();
                self.state.next += 1;
                if self.plays(step.condition) {
                    self.run_step(library, sd, &step);
                }
            }
            self.run_tasks();
        }
        self.move_camera_clip(library);
        self.follow_all(library);
        if frame >= 0 {
            self.move_debris();
        }
    }

    fn move_motions(&mut self, library: &mut Library) {
        for who in 0..2 {
            let asset = self.cast[who].clone();
            let state = &mut self.state.pokemon[who];
            state.clock += 1.0;
            let frames = f32::from(asset.motion_frames(state.motion.index).unwrap_or(0));
            let ended = advance(&mut state.motion, frames);
            if ended {
                if let Some(next) = state.reserve.take() {
                    state.motion = Motion { index: next, frame: 0.0, speed: state.motion.speed, repeat: true };
                }
            }
        }
        for model in self.state.models.values_mut() {
            if let Some((file, motion)) = &mut model.motion {
                let frames = library.motions(*file).and_then(|pack| pack.motion(motion.index).ok()).map_or(0.0, |m| f32::from(m.frames));
                motion.speed = model.speed;
                advance(motion, frames);
            }
            for animation in &mut model.animations {
                let length = library.animation(animation.file).map_or(0.0, |look| look.length);
                animation.frame += model.speed;
                if length > 0.0 && animation.frame >= length {
                    animation.frame = if animation.repeat { animation.frame % length } else { length };
                }
            }
        }
    }

    /// Starts a motion of a Pokémon. A motion that the Pokémon does not have leaves the one that plays.
    fn play(&mut self, who: usize, motion: usize, repeat: bool, next: Option<usize>) {
        if self.cast[who].motion_frames(motion).is_none() {
            return;
        }
        let state = &mut self.state.pokemon[who];
        state.motion = Motion { index: motion, frame: 0.0, speed: state.motion.speed, repeat };
        state.reserve = next;
    }

    /// True when a step with this condition plays in a single battle.
    fn plays(&self, condition: u16) -> bool {
        let near = |who: usize| Self::is_near(self.setup, who);
        let (attacker, defender) = (near(ATTACKER), near(self.aimed()));
        match condition {
            10 | 11 | 13 | 14 | 15 => false,
            20 => attacker,
            21 => !attacker,
            22 => defender,
            23 => !defender,
            24 => attacker && defender,
            25 => attacker && !defender,
            26 => !attacker && defender,
            27 => !attacker && !defender,
            30 => defender && self.cast[self.aimed()].size == 2,
            40 => self.state.pokemon[self.aimed()].charging_visible,
            41 => !self.cast[ATTACKER].no_jump,
            42 => self.cast[ATTACKER].no_jump,
            43 => !self.cast[self.aimed()].no_jump,
            44 => self.cast[self.aimed()].no_jump,
            // A contest: the first category, the smallest crowd and the first rank.
            51..=54 | 55 | 61..=63 | 65..=67 | 71 => false,
            _ => true,
        }
    }

    /// The matrices of the bones of a Pokémon in the pose of its motion, from the space of its model.
    fn bones(&mut self, who: usize) -> Option<(Rc<MotionPack>, Rc<Vec<Affine>>)> {
        let asset = self.cast[who].clone();
        let pack = asset.motions.as_ref()?;
        let motion = self.state.pokemon[who].motion;
        let key = (who, motion.index, motion.frame.to_bits());
        let worlds = match self.nodes.get(&key) {
            Some(found) => found.clone(),
            None => {
                let pose = pack.motion(motion.index).ok().map(|m| m.pose(pack.joints.len(), motion.frame));
                let worlds = Rc::new(match pose {
                    Some(pose) => posed_worlds(&asset.model, pack, &pose),
                    None => rest_worlds(&asset.model.bones),
                });
                if self.nodes.len() > 512 {
                    self.nodes.clear();
                }
                self.nodes.insert(key, worlds.clone());
                worlds
            }
        };
        // The pack is shared: only its names and points are used.
        Some((Rc::new(MotionPack::clone(pack)), worlds))
    }

    /// The matrix from the space of the model of a Pokémon to the stage.
    pub fn pokemon_matrix(&self, who: usize) -> Affine {
        let state = &self.state.pokemon[who];
        let mut place = math::add(state.object.position, state.object.offset);
        let mut turn = state.object.rotation;
        // A Pokémon that does not leave the ground does not lean.
        if self.cast[who].no_jump {
            place[1] = place[1].min(0.0);
            (turn[0], turn[2]) = (0.0, 0.0);
        }
        math::matrix(place, turn, math::scaled(state.object.scale, self.cast[who].model_scale()))
    }

    /// The place on the stage of a named point of a Pokémon.
    fn named_point(&mut self, who: usize, name: &str) -> Option<V3> {
        let asset = self.cast[who].clone();
        let node = asset.motions.as_ref()?.node(name)?;
        let (pack, worlds) = self.bones(who)?;
        let bone = bone_of_joints(&asset.model.bones, &pack).get(node.joint).copied().flatten()?;
        let local = worlds.get(bone)?.point(node.offset);
        Some(self.pokemon_matrix(who).point(local))
    }

    /// The place of a point of a Pokémon, by the number that the steps give it.
    pub fn point(&mut self, who: usize, point: i32) -> V3 {
        let origin = self.state.pokemon[who].object.position;
        if point == 0 {
            return origin;
        }
        // The point of an attack depends on the attack motion that plays, or that will play.
        let attack = || {
            let now = self.state.pokemon[who].motion.index;
            let motion = if (motions::PHYSICAL_1..=motions::SPECIAL_4).contains(&now) { now } else { self.attack.map_or(now, |(motion, _)| motion) };
            match motion {
                m if (motions::PHYSICAL_1..motions::PHYSICAL_1 + 4).contains(&m) => format!("Attack0{}_01", m - motions::PHYSICAL_1 + 1),
                m if (motions::PHYSICAL_1 + 4..=motions::SPECIAL_4).contains(&m) => format!("Shoot0{}_01", m - motions::PHYSICAL_1 - 3),
                _ => "Front01".to_string(),
            }
        };
        let (name, other) = match point {
            1 | 2 => (attack(), "Front01"),
            3 => ("Waist".to_string(), "Center01"),
            4 => ("OverHead01".to_string(), "Front01"),
            6 => ("HeadCenter01".to_string(), "Front01"),
            7 => ("Eye01".to_string(), "HeadCenter01"),
            8 => ("Mouth01".to_string(), "HeadCenter01"),
            9 => ("Horn01".to_string(), "Front01"),
            10 => ("Tail01".to_string(), "Front01"),
            11 => ("Hand01".to_string(), "Front01"),
            12 => ("Hand02".to_string(), "Front01"),
            13 => ("Foot01".to_string(), "Front01"),
            14 => ("Foot02".to_string(), "Front01"),
            15 => ("Center01".to_string(), "Front01"),
            20 => ("Jaw".to_string(), "Front01"),
            _ => ("Front01".to_string(), "Front01"),
        };
        let mut found = self.named_point(who, &name);
        // The eyes are between the two eyes.
        if point == 7 {
            if let Some(second) = self.named_point(who, "Eye02") {
                found = Some(found.map_or(second, |first| math::scaled(math::add(first, second), 0.5)));
            }
        }
        let mut place = found.or_else(|| self.named_point(who, other)).unwrap_or(origin);
        // Two Pokémon have a point that the game moves a little.
        let choice = self.cast[who].choice;
        let extra = match (choice.species, choice.form, point) {
            (384, 1, 15) => Some([0.0, 30.0, 70.0]),
            (302, 1, 5) => Some([0.0, 0.0, 35.0]),
            _ => None,
        };
        if let Some(mut extra) = extra {
            math::turn_xz(&mut extra, self.state.pokemon[who].object.rotation[1]);
            place = math::add(place, extra);
        }
        place
    }

    /// The place of a point of a Pokémon with an offset that follows the side of the battle, and the factors of the
    /// Pokémon. Gives the place, the turned offset, the turn of the Pokémon around the vertical axis, and the factors.
    fn relative(&mut self, who: usize, point: i32, offset: V3, option: i32, camera: bool) -> (V3, V3, f32, V3) {
        let mut offset = offset;
        // The point 16 is the place of the Pokémon at rest, with no factor.
        if point == 16 {
            let (place, turn) = self.home(who);
            math::turn_xz(&mut offset, -turn.to_radians());
            if self.reversed() {
                offset[2] = -offset[2];
            }
            return (place, offset, turn.to_radians(), [1.0; 3]);
        }
        let free = option & 8 != 0;
        // A Pokémon on the side of the attacker's point of view turns the offsets by half a turn.
        let turn = if !free && self.reversed() != Self::is_near(self.setup, who) { 3.1415 } else { 0.0 };
        let place = self.point(who, point);
        let asset = &self.cast[who];
        let factor = [if camera { asset.camera_scale() } else { asset.effect_scale() }; 3];
        math::turn_xz(&mut offset, -turn);
        if option & 1 != 0 {
            offset = math::mul(offset, factor);
        }
        if !free && self.reversed() {
            offset[2] = -offset[2];
        }
        let size = if option & 1 != 0 { factor } else { [1.0; 3] };
        (place, offset, turn, size)
    }

    /// The turns that make something at `from` look at a point of a Pokémon, from the turns that it has.
    fn toward(&mut self, from: V3, who: usize, point: i32, turn: V3, tilt: bool) -> V3 {
        let target = self.point(who, point);
        let way = math::sub(target, from);
        let mut turn = turn;
        let mut around = way[0].atan2(way[2]);
        if around - turn[1] > PI {
            around -= 2.0 * PI;
        }
        if around - turn[1] < -PI {
            around += 2.0 * PI;
        }
        turn[1] = around;
        if tilt {
            turn[0] = -way[1].atan2(way[0].hypot(way[2]));
        }
        turn
    }

    /// A place and an offset that follow the line from a Pokémon to another one.
    #[allow(clippy::too_many_arguments)]
    fn aligned(&mut self, who: usize, point: i32, other: usize, other_point: i32, offset: V3, tilt: bool, option: i32, camera: bool) -> (V3, V3, V3, V3) {
        let start = self.state.pokemon[who].object.rotation;
        let place = self.point(who, point);
        let turn = self.toward(place, other, other_point, start, tilt);
        let mut offset = offset;
        if self.reversed() {
            offset[0] = -offset[0];
        }
        math::turn_xz(&mut offset, -turn[1]);
        let mut size = [1.0; 3];
        if option & 1 != 0 {
            let asset = &self.cast[who];
            size = [if camera { asset.camera_scale() } else { asset.effect_scale() }; 3];
            offset = math::mul(offset, size);
        }
        (place, offset, turn, size)
    }

    fn object(&mut self, target: Ref) -> Option<&mut Object> {
        match target {
            Ref::Pokemon(who) => self.state.pokemon.get_mut(who).map(|p| &mut p.object),
            Ref::Effect(group, serial) => self.state.effects.get_mut(&group).filter(|e| e.serial == serial).map(|e| &mut e.object),
            Ref::Model(group, serial) => self.state.models.get_mut(&group).filter(|m| m.serial == serial).map(|m| &mut m.object),
            Ref::Debris(group, serial) => self.state.debris.get_mut(&group).filter(|d| d.serial == serial).map(|d| &mut d.object),
        }
    }

    /// Gives a property of an object a new value, at once or over the frames of a step.
    fn change(&mut self, target: Ref, property: Property, value: V3, frames: i32) {
        let Some(object) = self.object(target) else { return };
        let field = match property {
            Property::Position => &mut object.position,
            Property::Offset => &mut object.offset,
            Property::Rotation => &mut object.rotation,
            Property::Scale => &mut object.scale,
        };
        if frames <= 0 {
            *field = value;
        } else {
            let tween = Tween::new(*field, value, frames as u32, Curve::Linear);
            self.state.tasks.push(Task::Change { target, property, tween });
        }
    }

    fn effect_ref(&self, group: u16) -> Option<Ref> {
        self.state.effects.get(&group).map(|e| Ref::Effect(group, e.serial))
    }

    fn model_ref(&self, group: u16) -> Option<Ref> {
        self.state.models.get(&group).map(|m| Ref::Model(group, m.serial))
    }

    /// Places an effect or a model on a point of a Pokémon: the rules of the steps "Glide To Pokemon".
    fn place_on(&mut self, target: Ref, step: &Step, who: usize, frames: i32) {
        let option = int(step, "adjust");
        let (place, offset, turn, size) = self.relative(who, int(step, "point"), vector(step, "offset"), option, false);
        let flip = self.reversed() && option & 4 == 0 && option & 8 == 0;
        let Some(object) = self.object(target) else { return };
        if flip {
            object.flip = true;
        }
        if option & 1 != 0 {
            object.adjust = size.map(|s| (s - 1.0) * 0.5 + 1.0);
            object.rotation[1] = turn;
        }
        self.change(target, Property::Position, math::add(place, offset), frames);
    }

    /// The same, turned to the camera.
    fn place_toward_camera(&mut self, target: Ref, step: &Step, who: usize, frames: i32) {
        let option = int(step, "adjust");
        let mut offset = vector(step, "offset");
        let (place, _, _, size) = self.relative(who, int(step, "point"), offset, option, false);
        let way = math::sub(self.state.camera.position, place);
        let turn = way[0].atan2(way[2]) + float(step, "rotation_offset").to_radians();
        math::turn_xz(&mut offset, -turn);
        let Some(object) = self.object(target) else { return };
        if option & 1 != 0 {
            object.adjust = size.map(|s| (s - 1.0) * 0.5 + 1.0);
            object.rotation[1] = turn;
        }
        self.change(target, Property::Position, math::add(place, offset), frames);
    }

    /// Places an object on the line between two Pokémon, turned along it.
    fn place_along(&mut self, target: Ref, step: &Step, frames: i32) {
        let option = int(step, "adjust");
        let (who, other) = (self.who(int(step, "target")), self.who(int(step, "direction_target")));
        let (place, offset, turn, size) = self.aligned(who, int(step, "point"), other, int(step, "direction_point"), vector(step, "offset"), flag(step, "vertical"), option, false);
        let Some(object) = self.object(target) else { return };
        if option & 1 != 0 {
            object.adjust = size.map(|s| (s - 1.0) * 0.5 + 1.0);
        }
        self.change(target, Property::Position, math::add(place, offset), frames);
        self.change(target, Property::Rotation, turn, frames);
    }

    /// Places an object on a side of the field.
    fn place_on_field(&mut self, target: Ref, step: &Step, frames: i32) {
        let who = match int(step, "point") {
            0 => ATTACKER,
            _ => self.aimed(),
        };
        let place = self.home(who).0;
        let mut offset = vector(step, "offset");
        let reversed = self.reversed();
        if reversed {
            offset[2] = -offset[2];
        }
        let Some(object) = self.object(target) else { return };
        if reversed {
            object.flip = true;
        }
        self.change(target, Property::Position, math::add(place, offset), frames);
    }

    /// Turns an object to a point of a Pokémon.
    fn turn_to(&mut self, target: Ref, step: &Step, frames: i32) {
        let who = self.who(int(step, "target"));
        let Some(object) = self.object(target) else { return };
        let (from, now) = (object.position, object.rotation);
        let turn = self.toward(from, who, int(step, "point"), now, flag(step, "vertical"));
        let Some(object) = self.object(target) else { return };
        object.flip = false;
        self.change(target, Property::Rotation, turn, frames);
    }

    /// Turns an object by an angle.
    fn turn_by(&mut self, target: Ref, step: &Step, frames: i32) {
        let Some(object) = self.object(target) else { return };
        let mut turn = math::radians(vector(step, "degrees"));
        if matches!(target, Ref::Effect(..)) && object.flip {
            turn[0] = -turn[0];
        }
        let turn = math::add(turn, object.rotation);
        self.change(target, Property::Rotation, turn, frames);
    }

    /// Puts an object at a place of the stage.
    fn teleport(&mut self, target: Ref, step: &Step, frames: i32) {
        let reversed = self.reversed() && flag(step, "mirror");
        let Some(object) = self.object(target) else { return };
        if reversed {
            object.flip = true;
        }
        self.change(target, Property::Position, vector(step, "position"), frames);
    }

    fn shift(&mut self, target: Ref, offset: V3, frames: i32) {
        let Some(object) = self.object(target) else { return };
        let place = math::add(object.position, offset);
        self.change(target, Property::Position, place, frames);
    }

    fn glide_to_model(&mut self, target: Ref, step: &Step, frames: i32) {
        let mut offset = vector(step, "offset");
        if flag(step, "flip") && self.reversed() {
            offset[2] = -offset[2];
        }
        let Some(model) = self.state.models.get(&(int(step, "model_group") as u16)) else { return };
        let place = math::add(model.object.position, offset);
        self.change(target, Property::Position, place, frames);
    }

    fn shake(&mut self, target: Ref, step: &Step, frames: i32) {
        let steps = frames.max(0) as u32;
        let angle = Tween::new([float(step, "start_decay") * 360.0], [float(step, "end_decay") * 360.0], steps, Curve::Linear);
        let amount = Tween::new([float(step, "start_amount")], [float(step, "end_amount")], steps, Curve::Linear);
        self.state.tasks.push(Task::Shake { target, axis: int(step, "axis").clamp(0, 2) as usize, angle, amount });
    }

    /// The place at a distance in front of the camera, and the turns that face it.
    fn before_camera(&self, distance: f32) -> (V3, V3) {
        let camera = &self.state.camera;
        let eye = math::add(camera.position, camera.offset);
        let back = math::sub(eye, camera.target);
        let place = math::sub(eye, math::scaled(math::normalized(back), distance));
        let turn = [back[1].atan2(-back[0].hypot(back[2])), back[0].atan2(back[2]), 0.0];
        (place, turn)
    }

    fn follow_camera(&mut self, target: Ref, step: &Step, frames: i32) {
        let (distance, turn) = (float(step, "distance"), flag(step, "rotation_flags"));
        if frames > 0 {
            self.state.tasks.push(Task::FollowCamera { target, distance, turn, left: frames });
            return;
        }
        let (place, angles) = self.before_camera(distance);
        if let Some(object) = self.object(target) {
            object.position = place;
            if turn {
                object.rotation = angles;
            }
        }
    }

    /// Makes an object follow a bone of a model, or a point of a Pokémon.
    fn follow(&mut self, target: Ref, step: &Step) {
        let group = int(step, "model_group");
        let name = text(step, "bone_name");
        let follow = match group {
            // A trainer is not on this stage.
            -1 | 100 | 101 => Follow::None,
            _ if name.is_empty() => Follow::Model { group: group as u16 },
            _ => Follow::Bone { group: group as u16, name },
        };
        let (place, turn, axes) = (vector(step, "position_offset"), math::radians(vector(step, "rotation_offset")), int(step, "rotation"));
        if let Some(object) = self.object(target) {
            object.follow = follow;
            (object.follow_place, object.follow_turn, object.follow_axes) = (place, turn, axes);
        }
    }

    fn follow_pokemon(&mut self, target: Ref, step: &Step) {
        let (who, point, on) = (self.who(int(step, "target")), int(step, "point"), flag(step, "enabled"));
        let place = vector(step, "position_offset");
        if let Some(object) = self.object(target) {
            object.follow = if on { Follow::Pokemon { who, point } } else { Follow::None };
            (object.follow_place, object.follow_turn, object.follow_axes) = (place, [0.0; 3], 0);
        }
    }

    /// The places of the bones of a model on the stage: the matrix of the bone of this name.
    fn model_bone(&self, library: &mut Library, group: u16, name: &str) -> Option<Affine> {
        let model = self.state.models.get(&group)?;
        let file = library.model(model.number)?;
        let bone = file.model.bones.iter().position(|b| b.name == name)?;
        let moved = model.motion.and_then(|(motions, motion)| {
            let pack = library.motions(motions)?;
            let pose: Pose = pack.motion(motion.index).ok()?.pose(pack.joints.len(), motion.frame);
            Some(posed_worlds(&file.model, &pack, &pose))
        });
        let worlds = moved.unwrap_or_else(|| rest_worlds(&file.model.bones));
        Some(model.object.matrix().then(worlds.get(bone)?))
    }

    /// Moves every object that follows something, then the ones that turn with their movement.
    fn follow_all(&mut self, library: &mut Library) {
        let models: Vec<Ref> = self.state.models.iter().map(|(&g, m)| Ref::Model(g, m.serial)).collect();
        let effects: Vec<Ref> = self.state.effects.iter().map(|(&g, e)| Ref::Effect(g, e.serial)).collect();
        for target in [Ref::Pokemon(0), Ref::Pokemon(1)].into_iter().chain(models).chain(effects) {
            let Some(object) = self.object(target) else { continue };
            let (follow, offset, extra, axes) = (object.follow.clone(), object.follow_place, object.follow_turn, object.follow_axes);
            match follow {
                Follow::None => {}
                Follow::Pokemon { who, point } => {
                    let place = self.point(who, point);
                    if let Some(object) = self.object(target) {
                        object.position = place;
                    }
                }
                Follow::Model { group } => {
                    if let Some(place) = self.state.models.get(&group).map(|m| math::add(m.object.position, offset)) {
                        if let Some(object) = self.object(target) {
                            object.position = place;
                        }
                    }
                }
                Follow::Bone { group, name } => {
                    let Some(bone) = self.model_bone(library, group, &name) else { continue };
                    let place = math::add([bone.0[0][3], bone.0[1][3], bone.0[2][3]], offset);
                    let column = |c: usize| [bone.0[0][c], bone.0[1][c], bone.0[2][c]];
                    let angle = |y: f32, x: f32| if y == 0.0 { 0.0 } else { y.atan2(x) };
                    let (x, y, z) = (column(0), column(1), column(2));
                    let mut turn = [angle(y[2], y[1]), angle(z[0], z[2]), angle(x[1], x[0])];
                    if let Some(object) = self.object(target) {
                        object.position = place;
                        if axes != 0 {
                            for (axis, value) in turn.iter_mut().enumerate() {
                                if axes & (1 << axis) == 0 {
                                    *value = 0.0;
                                }
                            }
                            object.rotation = math::add(turn, extra);
                        }
                    }
                }
            }
            if let Some(object) = self.object(target) {
                if object.auto_rotate {
                    let now = math::add(object.position, object.offset);
                    let way = math::sub(now, object.before);
                    object.before = now;
                    object.rotation = [way[0].hypot(way[2]).atan2(way[1]), way[0].atan2(way[2]), 0.0];
                }
            }
        }
        // An effect leaves at the end of its time.
        let frame = self.state.frame;
        self.state.effects.retain(|_, effect| effect.life.is_none_or(|life| frame - effect.born < life));
    }

    fn move_debris(&mut self) {
        for debris in self.state.debris.values_mut() {
            let (place, turns, size) = (debris.object.position, debris.object.rotation, math::mul(debris.object.scale, debris.object.adjust));
            let factors = debris.object.adjust;
            debris.set.advance(place, turns, size, factors);
        }
        self.state.debris.retain(|_, debris| !debris.set.is_over());
    }

    /// Puts the camera where the camera animation is, then moves the animation one frame on.
    fn move_camera_clip(&mut self, library: &mut Library) {
        let Some(mut clip) = self.state.clip else { return };
        let Some(file) = library.camera(clip.file) else { return };
        let shot = file.at(clip.frame);
        let camera = &mut self.state.camera;
        camera.position = clip.place(shot.position);
        camera.target = clip.place(shot.target);
        camera.up = if clip.up_mirrored { [-shot.up[0], shot.up[1], -shot.up[2]] } else { shot.up };
        if shot.angle > 0.0 {
            camera.angle = shot.angle;
        }
        clip.frame += 1.0;
        if clip.frame >= file.frames {
            clip.frame = if file.repeat && file.frames > 0.0 { clip.frame - file.frames } else { file.frames };
        }
        self.state.clip = Some(clip);
    }

    fn run_tasks(&mut self) {
        let tasks = std::mem::take(&mut self.state.tasks);
        let mut kept = Vec::with_capacity(tasks.len());
        for mut task in tasks {
            let done = match &mut task {
                Task::Change { target, property, tween } => {
                    let (value, done) = tween.advance();
                    let property = *property;
                    match self.object(*target) {
                        Some(object) => {
                            match property {
                                Property::Position => object.position = value,
                                Property::Offset => object.offset = value,
                                Property::Rotation => object.rotation = value,
                                Property::Scale => object.scale = value,
                            }
                            done
                        }
                        None => true,
                    }
                }
                Task::Shake { target, axis, angle, amount } => {
                    let ((angle, first), (amount, second)) = (angle.advance(), amount.advance());
                    let axis = *axis;
                    match self.object(*target) {
                        Some(object) => {
                            object.offset[axis] = angle[0].to_radians().sin() * amount[0];
                            first && second
                        }
                        None => true,
                    }
                }
                Task::FollowCamera { target, distance, turn, left } => {
                    *left -= 1;
                    let (place, angles) = self.before_camera(*distance);
                    let (turn, done) = (*turn, *left <= 0);
                    match self.object(*target) {
                        Some(object) => {
                            object.position = place;
                            if turn {
                                object.rotation = angles;
                            }
                            done
                        }
                        None => true,
                    }
                }
                Task::Arc { target, speed, gravity, height, end } => {
                    *height += *speed;
                    *speed += *gravity;
                    let over = (*gravity < 0.0 && *speed < 0.0 && *height < *end) || (*gravity > 0.0 && *speed > 0.0 && *height > *end) || *gravity == 0.0;
                    let (height, end) = (*height, *end);
                    match self.object(*target) {
                        Some(object) => {
                            object.offset[1] = if over { 0.0 } else { height };
                            if over {
                                object.position[1] += end;
                            }
                            over
                        }
                        None => true,
                    }
                }
                Task::CameraMove { position, target, angle } => {
                    let ((position, done), (target, _), (angle, _)) = (position.advance(), target.advance(), angle.advance());
                    self.state.camera.position = position;
                    self.state.camera.target = target;
                    self.state.camera.angle = angle[0];
                    done
                }
                Task::CameraShake { axis, angle, amount } => {
                    let ((angle, first), (amount, second)) = (angle.advance(), amount.advance());
                    self.state.camera.offset = [0.0; 3];
                    self.state.camera.offset[*axis] = angle[0].to_radians().sin() * amount[0];
                    first && second
                }
                Task::CameraTurn { target, height, radius, angle } => {
                    let (angle, done) = angle.advance();
                    let mut place = [0.0, *height, *radius];
                    math::turn_xz(&mut place, angle[0]);
                    place[0] += target[0];
                    place[2] += target[2];
                    self.state.camera.position = place;
                    self.state.camera.target = *target;
                    done
                }
                Task::PokemonTint { who, color, strength } => {
                    let ((color, first), (strength, second)) = (color.advance(), strength.advance());
                    let state = &mut self.state.pokemon[*who];
                    state.tint = color;
                    state.tint_strength = strength[0];
                    first && second
                }
                Task::Fill { color } => {
                    let (color, done) = color.advance();
                    self.state.screen.fill = color;
                    done
                }
                Task::Colour { target, material, index, color } => {
                    let (color, done) = color.advance();
                    match *target {
                        Ref::Model(group, serial) => match self.state.models.get_mut(&group).filter(|m| m.serial == serial) {
                            Some(model) => {
                                set_colour(model, material, *index, color);
                                done
                            }
                            None => true,
                        },
                        _ => true,
                    }
                }
                Task::Delete { target, left } => {
                    *left -= 1;
                    if *left <= 0 {
                        if let Ref::Model(group, serial) = *target {
                            if self.state.models.get(&group).is_some_and(|m| m.serial == serial) {
                                self.state.models.remove(&group);
                            }
                        }
                    }
                    *left <= 0
                }
                Task::ClipFollow { who, point, offset, option, left } => {
                    let (place, _, _, _) = self.relative(*who, *point, *offset, *option, true);
                    if let Some(clip) = &mut self.state.clip {
                        clip.offset = place;
                    }
                    *left -= 1;
                    *left < -1
                }
                Task::Knockback { who, tween, returning } => {
                    let (offset, done) = tween.advance();
                    self.state.pokemon[*who].object.offset = offset;
                    if done && !*returning {
                        *tween = Tween::new(offset, [0.0; 3], KNOCKBACK_RETURN, Curve::Linear);
                        *returning = true;
                        false
                    } else {
                        done
                    }
                }
                Task::Fade { color } => {
                    let (color, done) = color.advance();
                    self.state.screen.fade = color;
                    done
                }
            };
            if !done {
                kept.push(task);
            }
        }
        kept.append(&mut self.state.tasks);
        self.state.tasks = kept;
    }

    /// Moves the camera to a place, at once or over the frames of a step. An angle of 0 keeps the angle.
    fn move_camera(&mut self, position: V3, target: V3, angle: f32, frames: i32, curve: i32) {
        let camera = &mut self.state.camera;
        if frames <= 0 {
            camera.position = position;
            camera.target = target;
            if angle != 0.0 {
                camera.angle = angle;
            }
            return;
        }
        let steps = frames as u32;
        let end_angle = if angle == 0.0 { camera.angle } else { angle };
        let task = Task::CameraMove {
            position: Tween::new(camera.position, position, steps, Curve::of(curve)),
            target: Tween::new(camera.target, target, steps, Curve::of(curve)),
            angle: Tween::new([camera.angle], [end_angle], steps, Curve::Linear),
        };
        self.state.tasks.push(task);
    }

    fn default_camera(&self) -> Camera {
        let (near, far) = if self.reversed() { (TARGET, ATTACKER) } else { (ATTACKER, TARGET) };
        default_camera(self.cast[near].size, self.cast[far].size)
    }

    fn debris_step(&mut self, library: &mut Library, name: &str, step: &Step, group: u16, frames: i32) {
        if name == "Spawn" {
            let number = int(step, "model");
            library.model(number);
            self.state.serial += 1;
            let layer = match int(step, "draw_mode") {
                1 => Layer::WithEffects,
                2 => Layer::BeforePokemon,
                _ => Layer::AfterPokemon,
            };
            let setup = debris::Setup {
                length: frames,
                most: int(step, "max_count").max(0) as usize,
                interval: int(step, "interval"),
                per_time: int(step, "count_per_emit"),
                rate: int(step, "rate"),
                life: int(step, "life"),
                shape: int(step, "shape"),
                axis: int(step, "axis"),
                size: vector(step, "size"),
                degrees: (int(step, "start_degrees"), int(step, "end_degrees")),
                reach: int(step, "length"),
                turns_with_movement: flag(step, "auto_rotate"),
            };
            let seed = u64::from(group) << 32 | u64::from(self.state.serial);
            let debris = DebrisState { serial: self.state.serial, number, object: Object::default(), layer, animations: Vec::new(), set: Debris::new(setup, seed) };
            self.state.debris.insert(group, debris);
            return;
        }
        let Some(target) = self.state.debris.get(&group).map(|d| Ref::Debris(group, d.serial)) else { return };
        match name {
            "Teleport" => {
                let mut place = vector(step, "position");
                if flag(step, "mirror") && self.reversed() {
                    (place[0], place[2]) = (-place[0], -place[2]);
                }
                self.change(target, Property::Position, place, frames);
            }
            "Shift" => self.shift(target, vector(step, "offset"), frames),
            "Glide To Pokemon" => {
                let who = self.who(int(step, "target"));
                let option = int(step, "adjust");
                let (place, offset, turn, size) = self.relative(who, int(step, "point"), vector(step, "offset"), option, false);
                if option & 1 != 0 {
                    if let Some(object) = self.object(target) {
                        object.adjust = size.map(|s| (s - 1.0) * 0.5 + 1.0);
                        object.rotation[1] = turn;
                    }
                }
                self.change(target, Property::Position, math::add(place, offset), frames);
            }
            "Glide To Field Point" => self.place_on_field(target, step, frames),
            "Turn" => {
                let mut degrees = vector(step, "degrees");
                if flag(step, "mirror") && self.reversed() {
                    degrees[1] += 180.0;
                }
                let Some(object) = self.object(target) else { return };
                let turn = math::add(math::radians(degrees), object.rotation);
                self.change(target, Property::Rotation, turn, frames);
            }
            "Resize" => self.change(target, Property::Scale, never_flat(vector(step, "scale")), frames),
            _ => {
                let Some(debris) = self.state.debris.get_mut(&group) else { return };
                let set = &mut debris.set;
                match name {
                    "Spread" => {
                        set.speed = debris::Speed::Away { mode: int(step, "shape"), offset: vector(step, "offset") };
                        (set.start_speed, set.keep_speed) = (float(step, "speed"), float(step, "acceleration"));
                    }
                    "Velocity" => {
                        set.speed = debris::Speed::Along { direction: vector(step, "degrees"), random: int(step, "random_degrees") };
                        (set.start_speed, set.keep_speed) = (float(step, "speed"), float(step, "acceleration"));
                    }
                    "Gravity" => set.gravity = Some(debris::Gravity { mode: int(step, "mode"), place: vector(step, "position"), absolute: flag(step, "absolute"), power: float(step, "power") }),
                    "Reflection" => set.bounce = Some(debris::Bounce { place: float(step, "position"), absolute: flag(step, "absolute"), plane: int(step, "plane"), mode: int(step, "mode"), brake: float(step, "brake") }),
                    "Initial Size" => set.sizes = (vector(step, "minimum"), vector(step, "maximum")),
                    "Initial Turn" => set.turns = (vector(step, "minimum"), vector(step, "maximum")),
                    "Size Curve" => set.size_keys = keys(step),
                    "Turn Curve" => set.turn_keys = keys(step).map(|(frame, degrees)| (frame, math::radians(degrees))),
                    "Material Animation" | "Model Animation" => {
                        let (file, repeat) = (int(step, "animation"), flag(step, "loop"));
                        library.animation(file);
                        debris.animations.push((file, repeat));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Starts one step of the file.
    fn run_step(&mut self, library: &mut Library, sd: Option<&SdCard>, step: &Step) {
        let Some(kind) = kind_of(step.kind) else { return };
        let name = kind.name;
        let frames = step.end - step.start;
        let group = step.group;
        if let Some(rest) = name.strip_prefix("Pokemon ") {
            self.pokemon_step(rest, step, frames);
        } else if let Some(rest) = name.strip_prefix("Camera ") {
            self.camera_step(library, rest, step, frames);
        } else if let Some(rest) = name.strip_prefix("Effect ") {
            self.effect_step(library, sd, rest, step, group, frames);
        } else if let Some(rest) = name.strip_prefix("Model ") {
            self.model_step(library, rest, step, group, frames);
        } else if let Some(rest) = name.strip_prefix("Debris ") {
            self.debris_step(library, rest, step, group, frames);
        } else {
            let screen = &mut self.state.screen;
            match name {
                "Backdrop Color Toggle" => {
                    screen.fill_on = flag(step, "enabled");
                    let color = vector(step, "color");
                    screen.fill = [color[0], color[1], color[2], float(step, "alpha")];
                }
                "Backdrop Color" => {
                    let color = vector(step, "color");
                    let color = [color[0], color[1], color[2], float(step, "alpha")];
                    if frames <= 0 {
                        screen.fill = color;
                    } else {
                        let tween = Tween::new(screen.fill, color, frames as u32, Curve::Linear);
                        self.state.tasks.push(Task::Fill { color: tween });
                    }
                }
                "Backdrop Show" => screen.place_shown = flag(step, "shown"),
                "Platform Show" => screen.plates_shown = flag(step, "shown"),
                "Screen Fade In" | "Screen Fade Out" => {
                    // The lower screen is not shown.
                    if int(step, "screen") == 1 {
                        return;
                    }
                    let level = if int(step, "color") == 0 { 1.0 } else { 0.0 };
                    let full = [level, level, level, 1.0];
                    let clear = [level, level, level, 0.0];
                    let (from, to) = if name == "Screen Fade In" { (full, clear) } else { (clear, full) };
                    let steps = if frames <= 0 { DEFAULT_FADE } else { frames as u32 };
                    screen.fade = from;
                    self.state.tasks.push(Task::Fade { color: Tween::new(from, to, steps, Curve::Linear) });
                }
                "Knockback Flag" => self.knock_back(self.who(int(step, "who")), flag(step, "use_default")),
                "Charge Hide" => {
                    let who = self.who(int(step, "who"));
                    self.state.pokemon[who].charging_visible = flag(step, "visible");
                }
                _ => {}
            }
        }
    }

    /// Pushes a Pokémon away from what it faces.
    fn knock_back(&mut self, who: usize, returns: bool) {
        let mut offset = [0.0, 0.0, -KNOCKBACK];
        math::turn_xz(&mut offset, -self.state.pokemon[who].object.rotation[1]);
        let offset = math::scaled(offset, self.cast[who].effect_scale());
        if returns {
            let tween = Tween::new([0.0; 3], offset, KNOCKBACK_AWAY, Curve::Linear);
            self.state.tasks.push(Task::Knockback { who, tween, returning: false });
        } else {
            self.change(Ref::Pokemon(who), Property::Offset, offset, KNOCKBACK_HELD);
            self.state.pokemon[who].pushed = true;
        }
    }

    /// A camera that starts again puts the Pokémon that stayed pushed back to their place.
    fn end_knockback(&mut self) {
        for state in &mut self.state.pokemon {
            if state.pushed {
                state.pushed = false;
                state.object.offset = [0.0; 3];
            }
        }
    }

    fn pokemon_step(&mut self, name: &str, step: &Step, frames: i32) {
        let who = self.who(int(step, "who"));
        let target = Ref::Pokemon(who);
        match name {
            "Teleport" => {
                let mut place = vector(step, "position");
                if flag(step, "mirror") && self.reversed() {
                    place[2] = -place[2];
                }
                self.change(target, Property::Position, place, frames);
            }
            "Shift" => self.shift(target, vector(step, "offset"), frames),
            "Glide To Pokemon" => {
                let destination = self.who(int(step, "destination"));
                let (place, offset) = if flag(step, "rotate") {
                    let (place, offset, _, _) = self.aligned(destination, 0, who, 0, vector(step, "offset"), false, int(step, "adjust"), false);
                    (place, offset)
                } else {
                    let (place, offset, _, _) = self.relative(destination, int(step, "point"), vector(step, "offset"), int(step, "adjust"), false);
                    (place, offset)
                };
                self.change(target, Property::Position, math::add(place, offset), frames);
            }
            "Reset Position" => {
                let (place, turn) = self.home(who);
                let mut angles = self.state.pokemon[who].object.rotation;
                angles[1] = turn.to_radians();
                if frames <= 0 {
                    angles = [0.0, turn.to_radians(), 0.0];
                }
                self.change(target, Property::Position, place, frames);
                self.change(target, Property::Rotation, angles, frames);
            }
            "Reset All" => {
                for who in 0..2 {
                    let (place, turn) = self.home(who);
                    let object = &mut self.state.pokemon[who].object;
                    object.position = place;
                    object.rotation = [0.0, turn.to_radians(), 0.0];
                }
            }
            "Glide To Model" => self.glide_to_model(target, step, frames),
            "Spin" => {
                let turn = math::add(math::radians(vector(step, "degrees")), self.state.pokemon[who].object.rotation);
                self.change(target, Property::Rotation, turn, frames);
            }
            "Face Pokemon" => {
                let other = self.who(int(step, "toward"));
                let object = &self.state.pokemon[who].object;
                let (from, now) = (object.position, object.rotation);
                let turn = self.toward(from, other, 0, now, false);
                self.change(target, Property::Rotation, turn, frames);
            }
            "Resize" => self.change(target, Property::Scale, never_flat(vector(step, "scale")), frames),
            "Show" => self.state.pokemon[who].visible = flag(step, "visible"),
            "Show Special" => {
                let high = self.cast[who].top * self.cast[who].model_scale();
                if high >= float(step, "height") {
                    self.state.pokemon[who].visible = flag(step, "visible");
                }
            }
            "Show Non Targets" => {
                // In a single battle, the only Pokémon that is not a target is the other one of a move on its user.
                if self.setup.targets_user {
                    self.state.pokemon[TARGET].visible = flag(step, "visible");
                }
            }
            "Show Shadow" => self.state.pokemon[who].shadow = flag(step, "visible"),
            "Play Pose" => {
                let pose = int(step, "pose");
                if pose == 0 {
                    if self.state.pokemon[who].motion.index != motions::WAIT {
                        self.play(who, motions::WAIT, true, None);
                    }
                } else if pose >= FIRST_ATTACK_POSE {
                    if let Some((motion, _)) = self.attack {
                        if motion != motions::WAIT {
                            self.play(who, motion, false, Some(motions::WAIT));
                        }
                    }
                } else if let Some(&motion) = usize::try_from(pose).ok().and_then(|p| POSES.get(p)) {
                    let next = match motion {
                        motions::LAND_A => Some(motions::LAND_B),
                        motions::FAINT => None,
                        _ => Some(motions::WAIT),
                    };
                    self.play(who, motion, false, next);
                }
            }
            "Play Pose Direct" => {
                let motion = int(step, "pose").max(0) as usize;
                let next = flag(step, "reset_wait").then_some(motions::WAIT);
                self.play(who, motion, false, next);
            }
            "Pose Speed" => self.state.pokemon[who].motion.speed = float(step, "speed"),
            "Tint" => {
                let (start, end) = (vector(step, "start_color"), vector(step, "end_color"));
                let (from, to) = (float(step, "start_strength"), float(step, "end_strength"));
                if frames <= 0 {
                    let state = &mut self.state.pokemon[who];
                    state.tint = end;
                    state.tint_strength = to;
                } else {
                    let steps = frames as u32;
                    self.state.tasks.push(Task::PokemonTint { who, color: Tween::new(start, end, steps, Curve::Linear), strength: Tween::new([from], [to], steps, Curve::Linear) });
                }
            }
            "Follow Model" => self.follow(target, step),
            "Stop Shake" => self.change(target, Property::Offset, [0.0; 3], frames),
            "Shake" => self.shake(target, step, frames),
            _ => {}
        }
    }

    /// The steps of the camera animations.
    fn clip_step(&mut self, library: &mut Library, name: &str, step: &Step, frames: i32) {
        match name {
            "Clip On Pokemon" => {
                let kept = if flag(step, "keep_transform") { self.state.clip } else { None };
                let file = int(step, "clip");
                if library.camera(file).is_none() {
                    return;
                }
                let mut clip = Clip::new(file);
                match kept {
                    Some(before) => {
                        (clip.offset, clip.turn, clip.mirrored) = (before.offset, before.turn, before.mirrored);
                        clip.scale = before.scale;
                    }
                    None => {
                        let who = self.who(int(step, "who"));
                        let option = int(step, "adjust");
                        let (place, _, turn, size) = self.relative(who, int(step, "point"), [0.0; 3], option, true);
                        let mut size = size;
                        if flag(step, "mirror") && self.reversed() {
                            size[2] = -size[2];
                        }
                        clip.offset = place;
                        clip.turn = turn;
                        clip.set_scale(size);
                    }
                }
                self.state.clip = Some(clip);
                self.end_knockback();
            }
            "Clip On Position" => {
                let file = int(step, "clip");
                if library.camera(file).is_none() {
                    return;
                }
                let mut clip = Clip::new(file);
                clip.offset = vector(step, "position");
                if flag(step, "force_mirror") {
                    clip.scale[2] = -1.0;
                }
                self.state.clip = Some(clip);
                self.end_knockback();
            }
            "Clip Follow" => {
                let who = self.who(int(step, "target"));
                let task = Task::ClipFollow { who, point: int(step, "point"), offset: vector(step, "offset"), option: int(step, "option"), left: frames.max(0) };
                self.state.tasks.push(task);
            }
            "Clip End" => self.state.clip = None,
            _ => {
                let Some(clip) = &mut self.state.clip else { return };
                match name {
                    "Clip Rotate" => clip.turn = (int(step, "degrees_y") as f32).to_radians(),
                    "Clip Scale" => clip.set_scale(vector(step, "scale")),
                    "Clip Scale Value" => clip.set_scale(math::scaled(clip.scale, float(step, "scale"))),
                    "Clip Mirror" => (clip.mirrored, clip.up_mirrored) = (flag(step, "mirror_x"), flag(step, "mirror_up_vector")),
                    _ => {}
                }
            }
        }
    }

    fn camera_step(&mut self, library: &mut Library, name: &str, step: &Step, frames: i32) {
        if name.starts_with("Clip ") && !name.starts_with("Clip Planes") {
            self.clip_step(library, name, step, frames);
            return;
        }
        // A move of the camera ends the camera animation.
        if matches!(name, "Jump" | "Shift" | "Glide To Pokemon" | "Lock" | "Orbit Pokemon" | "Restore") {
            self.state.clip = None;
        }
        let curve = int(step, "curve");
        match name {
            "Jump" => self.move_camera(vector(step, "position"), vector(step, "look_at"), float(step, "field_of_view"), frames, curve),
            "Shift" => {
                let camera = &self.state.camera;
                let (position, target) = (math::add(camera.position, vector(step, "position")), math::add(camera.target, vector(step, "look_at")));
                self.move_camera(position, target, float(step, "field_of_view"), frames, curve);
            }
            "Glide To Pokemon" => {
                let (target, option, point) = (int(step, "target"), int(step, "adjust"), int(step, "point"));
                let who = self.who(target);
                let (position, look_at) = if flag(step, "auto_rotate") {
                    let other = match target {
                        0 => self.aimed(),
                        _ => ATTACKER,
                    };
                    let (place, offset, _, _) = self.aligned(who, point, other, 0, vector(step, "camera_position"), false, option, true);
                    let position = math::add(place, offset);
                    let (place, offset, _, _) = self.aligned(who, point, other, 0, vector(step, "look_at"), false, option, true);
                    (position, math::add(place, offset))
                } else {
                    let (_, look, _, _) = self.relative(who, point, vector(step, "look_at"), option, true);
                    let (place, from, _, _) = self.relative(who, point, vector(step, "camera_position"), option, true);
                    (math::add(place, from), math::add(place, look))
                };
                self.move_camera(position, look_at, float(step, "field_of_view"), frames, curve);
            }
            "Lock" => {
                let who = if int(step, "target") == 0 { ATTACKER } else { self.aimed() };
                let object = &self.state.pokemon[who].object;
                let (place, turn) = (object.position, object.rotation[1]);
                let factor = self.cast[who].effect_scale();
                let look_at = [place[0], place[1] + 25.0 * factor, place[2]];
                let mut offset = [if self.reversed() { -30.0 } else { 30.0 }, 30.0, 75.0].map(|v| v * factor);
                math::turn_xz(&mut offset, turn);
                self.move_camera(math::add(place, offset), look_at, 40.0, frames, curve);
            }
            "Orbit Pokemon" => {
                let (who, other) = (self.who(int(step, "target")), self.who(int(step, "direction_target")));
                let (point, other_point, option, tilt) = (int(step, "point"), int(step, "direction_point"), int(step, "adjust"), flag(step, "vertical"));
                let (place, offset, _, _) = self.aligned(who, point, other, other_point, vector(step, "camera_position"), tilt, option, true);
                let position = math::add(place, offset);
                let (place, offset, _, _) = self.aligned(who, point, other, other_point, vector(step, "look_at"), tilt, option, true);
                self.move_camera(position, math::add(place, offset), float(step, "field_of_view"), frames, curve);
            }
            "Restore" => {
                let rest = self.default_camera();
                let (mut position, mut target) = (rest.position, rest.target);
                let turned = flag(step, "flip");
                if frames <= 0 {
                    if turned {
                        position[2] = -position[2];
                        target[2] = -target[2];
                    }
                    self.state.camera = Camera { position, target, ..rest };
                    self.end_knockback();
                } else {
                    if turned && self.reversed() {
                        position[2] = -position[2];
                        target[2] = -target[2];
                    }
                    self.move_camera(position, target, rest.angle, frames, curve);
                }
            }
            "Turn" => {
                let mut degrees = int(step, "degrees") as f32;
                if flag(step, "flip") && !Self::is_near(self.setup, self.who(int(step, "flip_target"))) {
                    degrees = -degrees;
                }
                let camera = &self.state.camera;
                let way = math::sub(camera.position, camera.target);
                let start = -way[0].atan2(way[2]);
                let curve = match int(step, "move_option") {
                    3 => Curve::EaseBack,
                    other => Curve::of(other),
                };
                let task = Task::CameraTurn { target: camera.target, height: camera.position[1], radius: way[0].hypot(way[2]), angle: Tween::new([start], [start + degrees.to_radians()], frames.max(0) as u32, curve) };
                self.state.tasks.push(task);
            }
            "Turn To Pokemon" => {
                let who = self.who(int(step, "toward"));
                let (position, target) = (self.state.camera.position, self.state.camera.target);
                let turn = self.toward(position, who, 0, [0.0; 3], false);
                let mut way = math::sub(target, position);
                math::turn_xz(&mut way, turn[1] + PI);
                way[0] = -way[0];
                self.move_camera(position, math::add(position, way), 0.0, frames, 0);
            }
            "Rumble" => {
                if frames > 0 {
                    let steps = frames as u32;
                    self.state.tasks.push(Task::CameraShake {
                        axis: int(step, "axis").clamp(0, 2) as usize,
                        angle: Tween::new([0.0], [float(step, "decay") * 360.0], steps, Curve::Linear),
                        amount: Tween::new([float(step, "start_amount")], [float(step, "end_amount")], steps, Curve::Linear),
                    });
                }
            }
            _ => {}
        }
    }

    fn effect_step(&mut self, library: &mut Library, sd: Option<&SdCard>, name: &str, step: &Step, group: u16, frames: i32) {
        if name == "Spawn" || name == "Spawn For Contest" {
            let number = int(step, if name == "Spawn" { "effect" } else { "cool" });
            // A rare effect only shows on a shiny Pokémon.
            if step.condition == 70 && flag(step, "per_target") && !self.cast[self.aimed()].choice.shiny {
                return;
            }
            library.effect(sd, number);
            self.state.serial += 1;
            let effect = EffectState { serial: self.state.serial, number, object: Object::default(), born: self.state.frame, life: (frames > 0).then_some(frames), stopped: None, behind: int(step, "layer") != 0, on_screen: false };
            self.state.effects.insert(group, effect);
            return;
        }
        let Some(target) = self.effect_ref(group) else { return };
        match name {
            "Stop Emitting" => {
                let frame = self.state.frame;
                if let Some(effect) = self.state.effects.get_mut(&group) {
                    effect.stopped = Some(frame);
                }
            }
            "Delete" => {
                self.state.effects.remove(&group);
            }
            "Teleport" => self.teleport(target, step, frames),
            "Shift" => self.shift(target, vector(step, "offset"), frames),
            "Glide To Pokemon" => {
                let who = self.who(int(step, "target"));
                self.place_on(target, step, who, frames);
            }
            "Glide To Pokemon Rotated" => {
                let who = self.who(int(step, "target"));
                self.place_toward_camera(target, step, who, frames);
            }
            "Glide To Pokemon Legacy" | "Glide To Pokemon Rotated Legacy" => {
                let who = self.who(int(step, "target"));
                let offset = math::add(vector(step, "temporary_offset"), vector(step, "effect_offset"));
                let (place, offset, turn, size) = self.relative(who, int(step, "point"), offset, int(step, "adjust"), false);
                if let Some(object) = self.object(target) {
                    if int(step, "adjust") & 1 != 0 {
                        object.adjust = size.map(|s| (s - 1.0) * 0.5 + 1.0);
                        object.rotation[1] = turn;
                    }
                }
                self.change(target, Property::Position, math::add(place, offset), frames);
            }
            "Glide To Field Point" => self.place_on_field(target, step, frames),
            "Orbit Pokemon" => self.place_along(target, step, frames),
            "Glide To Model" => self.glide_to_model(target, step, frames),
            "Resize" => self.change(target, Property::Scale, never_flat(vector(step, "scale")), frames),
            "Turn" => self.turn_by(target, step, frames),
            "Turn To Pokemon" => self.turn_to(target, step, frames),
            "Follow Camera" => self.follow_camera(target, step, frames),
            "Follow Model" => self.follow(target, step),
            "Follow Pokemon" => self.follow_pokemon(target, step),
            "Lock Display" => {
                let mirrored = flag(step, "mirror") && self.reversed();
                if let Some(effect) = self.state.effects.get_mut(&group) {
                    effect.on_screen = true;
                    effect.object.position = [0.0, 0.0, float(step, "distance")];
                    if mirrored {
                        effect.object.adjust[0] = -effect.object.adjust[0];
                    }
                }
            }
            "Auto Spin" => self.auto_spin(target, flag(step, "enabled")),
            "Stop Shake" => self.change(target, Property::Offset, [0.0; 3], frames),
            "Shake" => self.shake(target, step, frames),
            _ => {}
        }
    }

    fn auto_spin(&mut self, target: Ref, on: bool) {
        if let Some(object) = self.object(target) {
            if on && !object.auto_rotate {
                object.before = math::add(object.position, object.offset);
            }
            object.auto_rotate = on;
            object.flip = false;
        }
    }

    fn model_step(&mut self, library: &mut Library, name: &str, step: &Step, group: u16, frames: i32) {
        if name == "Spawn" {
            let number = int(step, "model");
            library.model(number);
            self.state.serial += 1;
            let layer = match int(step, "draw_mode") {
                1 => Layer::WithEffects,
                2 => Layer::BeforePokemon,
                _ => Layer::AfterPokemon,
            };
            let model = ModelState { serial: self.state.serial, number, object: Object::default(), layer, visible: true, motion: None, animations: Vec::new(), speed: 1.0, colours: Vec::new(), on_screen: false };
            let target = Ref::Model(group, model.serial);
            self.state.models.insert(group, model);
            if frames > 0 {
                self.state.tasks.push(Task::Delete { target, left: frames });
            }
            return;
        }
        let Some(target) = self.model_ref(group) else { return };
        match name {
            "Remove" => self.state.tasks.push(Task::Delete { target, left: frames }),
            "Teleport" => self.teleport(target, step, frames),
            "Shift" => self.shift(target, vector(step, "offset"), frames),
            "Glide To Pokemon" => {
                let who = self.who(int(step, "target"));
                self.place_on(target, step, who, frames);
            }
            "Glide To Pokemon Rotated" => {
                let who = self.who(int(step, "target"));
                self.place_toward_camera(target, step, who, frames);
            }
            "Glide To Field Point" => self.place_on_field(target, step, frames),
            "Orbit Pokemon" => self.place_along(target, step, frames),
            "Turn" => self.turn_by(target, step, frames),
            "Turn To Pokemon" => self.turn_to(target, step, frames),
            "Resize" => self.change(target, Property::Scale, never_flat(vector(step, "scale")), frames),
            "Reset Size" => {
                if let Some(object) = self.object(target) {
                    object.adjust = [1.0; 3];
                }
            }
            "Show" => {
                if let Some(model) = self.state.models.get_mut(&group) {
                    model.visible = flag(step, "visible");
                }
            }
            "Stick To Camera" => self.follow_camera(target, step, frames),
            "Stick To Model" => self.follow(target, step),
            "Follow Pokemon" => self.follow_pokemon(target, step),
            "Auto Spin" => self.auto_spin(target, flag(step, "enabled")),
            "Stop Shake" => self.change(target, Property::Offset, [0.0; 3], frames),
            "Shake" => self.shake(target, step, frames),
            "Arc" => {
                if let Some(object) = self.object(target) {
                    let start = object.position[1];
                    let end = float(step, "end_height") + if flag(step, "relative") { start } else { 0.0 };
                    self.state.tasks.push(Task::Arc { target, speed: float(step, "speed"), gravity: float(step, "gravity"), height: 0.0, end: end - start });
                }
            }
            "Lock Display" => {
                let mirrored = flag(step, "mirror") && self.reversed();
                if let Some(model) = self.state.models.get_mut(&group) {
                    model.on_screen = true;
                    model.object.position = [0.0, 0.0, float(step, "distance")];
                    model.object.flip |= mirrored;
                }
            }
            "Play Motion" => {
                let (file, index, repeat) = (int(step, "motion"), int(step, "motion_index"), flag(step, "loop"));
                library.motions(file);
                if let Some(model) = self.state.models.get_mut(&group) {
                    let speed = model.speed;
                    // The motion -1 only loads the file.
                    let index = usize::try_from(index).ok();
                    model.motion = Some((file, Motion { index: index.unwrap_or(usize::MAX), frame: 0.0, speed, repeat }));
                }
            }
            "Switch Motion" => {
                let (index, repeat) = (int(step, "motion_index"), flag(step, "loop"));
                if let Some((_, motion)) = self.state.models.get_mut(&group).and_then(|m| m.motion.as_mut()) {
                    (motion.index, motion.frame, motion.repeat) = (usize::try_from(index).unwrap_or(usize::MAX), 0.0, repeat);
                }
            }
            "Play Texture Animation" | "Play Visibility Animation" | "Play Shader Animation" => {
                let (file, repeat) = (int(step, "animation"), flag(step, "loop"));
                library.animation(file);
                if let Some(model) = self.state.models.get_mut(&group) {
                    model.animations.push(Animation { file, frame: 0.0, repeat });
                }
            }
            "Motion Speed" => {
                if let Some(model) = self.state.models.get_mut(&group) {
                    model.speed = float(step, "speed");
                }
            }
            "Tint" => {
                let material = text(step, "material_name");
                let index = int(step, "constant_index").clamp(0, 5) as usize;
                let color = vector(step, "color");
                let color = [color[0], color[1], color[2], float(step, "alpha")];
                let Some(model) = self.state.models.get_mut(&group) else { return };
                if frames <= 0 {
                    set_colour(model, &material, index, color);
                } else {
                    // The change starts from the colour that the material has now.
                    let now = model.colours.iter().find(|(m, i, _)| *m == material && *i == index).map(|(_, _, c)| *c).or_else(|| library.model(model.number).and_then(|file| Look::colour(&file.model, &material, index))).unwrap_or(color);
                    self.state.tasks.push(Task::Colour { target, material, index, color: Tween::new(now, color, frames as u32, Curve::Linear) });
                }
            }
            _ => {}
        }
    }
}

/// The five keys of a curve of debris: a frame and a value each.
fn keys(step: &Step) -> debris::Keys {
    let mut keys = debris::NO_KEYS;
    if let Some(Value::Bytes(bytes)) = value(step, "keys") {
        for (key, data) in keys.iter_mut().zip(bytes.chunks_exact(16)) {
            let word = |at: usize| [data[at], data[at + 1], data[at + 2], data[at + 3]];
            *key = (i32::from_le_bytes(word(0)), [4, 8, 12].map(|at| f32::from_le_bytes(word(at))));
        }
    }
    keys
}

/// Keeps the colour that a step gives to a material of a model.
fn set_colour(model: &mut ModelState, material: &str, index: usize, color: [f32; 4]) {
    match model.colours.iter_mut().find(|(m, i, _)| m == material && *i == index) {
        Some(found) => found.2 = color,
        None => model.colours.push((material.to_string(), index, color)),
    }
}

/// Moves a motion one frame on. Gives true when a motion that does not repeat has ended.
fn advance(motion: &mut Motion, frames: f32) -> bool {
    if frames <= 0.0 {
        return false;
    }
    motion.frame += motion.speed;
    if motion.repeat {
        motion.frame = motion.frame.rem_euclid(frames);
        false
    } else {
        let ended = motion.frame >= frames;
        motion.frame = motion.frame.clamp(0.0, frames);
        ended
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_change_over_frames_follows_its_curve_and_ends_on_the_value() {
        let mut linear = Tween::new([0.0], [10.0], 4, Curve::Linear);
        assert_eq!(linear.advance(), ([2.5], false));
        for _ in 0..2 {
            linear.advance();
        }
        assert_eq!(linear.advance(), ([10.0], true));
        let mut fast = Tween::new([0.0], [10.0], 2, Curve::FastStart);
        assert!(fast.advance().0[0] > 5.0);
        let mut slow = Tween::new([0.0], [10.0], 2, Curve::SlowStart);
        assert!(slow.advance().0[0] < 5.0);
    }

    #[test]
    fn a_motion_repeats_or_stops_at_its_end() {
        let mut motion = Motion { index: 0, frame: 9.0, speed: 2.0, repeat: true };
        assert!(!advance(&mut motion, 10.0));
        assert_eq!(motion.frame, 1.0);
        motion = Motion { frame: 9.0, repeat: false, ..motion };
        assert!(advance(&mut motion, 10.0));
        assert_eq!(motion.frame, 10.0);
    }
}
