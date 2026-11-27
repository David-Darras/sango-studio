//! A picture of the stage at a frame, in the order that the battle draws: the place of the battle, the colour that can
//! cover it, what is behind the Pokémon, the Pokémon, what is in front of them, the particles, then what stays in front
//! of the camera and the fade of the screen.

use crate::assets::{Library, ModelFile, Place};
use crate::math::{self, V3};
use crate::raster::{draw, Canvas, Item, Shade};
use crate::sim::{DebrisState, Layer, ModelState, Run, CAMERA_NEAR};
use sango_core::animation::Look;
use sango_core::model::Model;
use sango_core::posing::{posed, Affine};
use sango_core::SdCard;
use sango_effect::{Placed, View};
use std::rc::Rc;

/// A place of a battle that the viewer can show: its name, the numbers of the files of its ground, of the plate under each
/// Pokémon and of its far side (0 for none), and whether it is under the open sky.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surroundings {
    pub name: &'static str,
    pub ground: usize,
    pub plate: usize,
    pub far: usize,
    pub open: bool,
}

const fn place(name: &'static str, ground: usize, plate: usize, far: usize, open: bool) -> Surroundings {
    Surroundings { name, ground, plate, far, open }
}

/// The places, matched by the names of their pictures. The first one is the one that the viewer starts with.
pub const PLACES: [Surroundings; 34] = [
    place("Grass", 94, 150, 11, true),
    place("Forest", 97, 151, 13, true),
    place("Town", 107, 146, 11, true),
    place("City", 68, 117, 6, true),
    place("Beach", 88, 135, 8, true),
    place("Shallow water", 73, 115, 4, true),
    place("Pond", 96, 139, 12, true),
    place("Sea", 101, 142, 15, true),
    place("Desert", 100, 141, 14, true),
    place("Mountain", 112, 149, 17, true),
    place("Red rocks", 92, 137, 10, true),
    place("Ash field", 86, 133, 7, true),
    place("Volcano", 108, 147, 0, false),
    place("Sky", 105, 144, 0, true),
    place("Cave 1", 70, 118, 0, false),
    place("Cave 2", 71, 119, 0, false),
    place("Cave 3", 72, 120, 0, false),
    place("Cave 4", 75, 123, 0, false),
    place("Graveyard", 87, 134, 0, false),
    place("Room", 89, 136, 0, false),
    place("Factory", 76, 124, 0, false),
    place("Palace", 98, 140, 0, false),
    place("Gym 1", 78, 125, 0, false),
    place("Gym 2", 79, 126, 0, false),
    place("Gym 3", 80, 127, 0, false),
    place("Gym 4", 81, 128, 0, false),
    place("Gym 5", 82, 129, 0, false),
    place("Gym 6", 83, 130, 0, false),
    place("Gym 7", 84, 131, 0, false),
    place("Gym 8", 85, 132, 0, false),
    place("Champion room", 66, 116, 0, false),
    place("Elite Four room 1", 35, 0, 0, false),
    place("Link battle", 109, 148, 0, false),
    place("No place", 0, 0, 0, false),
];

impl Default for Surroundings {
    fn default() -> Self {
        PLACES[0]
    }
}

/// The colours of the sky of an open place, from the top of the picture to its bottom. The sky of the game changes with
/// the hour and the weather: this one is a clear day.
const SKY: [[f32; 3]; 2] = [[112.0, 176.0, 236.0], [196.0, 228.0, 248.0]];

/// The height of the ground.
const GROUND: f32 = -1.0;
/// The camera that shows what stays in front of it: at the origin, toward +z, with this angle (degrees).
const SCREEN_ANGLE: f32 = 40.0;
/// How dark the shadow of a Pokémon is.
const SHADOW: f32 = 0.4;

/// The point of view of the camera of a frame.
pub fn view_of(run: &Run) -> View {
    let camera = &run.state.camera;
    let eye = math::add(camera.position, camera.offset);
    let target = math::add(camera.target, camera.offset);
    let mut forward = math::normalized(math::sub(target, eye));
    if forward == [0.0; 3] {
        forward = [0.0, 0.0, -1.0];
    }
    let mut right = math::normalized(math::cross(forward, camera.up));
    if right == [0.0; 3] {
        right = math::normalized(math::cross(forward, [0.0, 0.0, -1.0]));
    }
    let up = math::cross(right, forward);
    let angle = camera.angle.clamp(1.0, 170.0);
    View { eye, right, up, forward, focal: 0.5 / (angle.to_radians() / 2.0).tan(), near: CAMERA_NEAR }
}

fn screen_view() -> View {
    View { eye: [0.0; 3], right: [-1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], forward: [0.0, 0.0, 1.0], focal: 0.5 / (SCREEN_ANGLE.to_radians() / 2.0).tan(), near: 1.0 }
}

/// A model with the pose and the look of its frame.
struct Dressed {
    file: Rc<ModelFile>,
    model: Model,
    matrix: Affine,
}

fn dress(library: &mut Library, state: &ModelState) -> Option<Dressed> {
    let file = library.model(state.number)?;
    let moved = state.motion.and_then(|(motions, motion)| {
        let pack = library.motions(motions)?;
        let pose = pack.motion(motion.index).ok()?.pose(pack.joints.len(), motion.frame);
        Some(posed(&file.model, &pack, &pose))
    });
    let mut model = moved.unwrap_or_else(|| file.model.clone());
    for animation in &state.animations {
        if let Some(look) = library.animation(animation.file) {
            look.apply(&mut model, animation.frame);
        }
    }
    for (material, index, color) in &state.colours {
        Look::set_colour(&mut model, material, *index, *color);
    }
    Some(Dressed { file, model, matrix: state.object.matrix() })
}

/// The copies of a set of debris, each with the animation of its own age.
fn copies(library: &mut Library, debris: &DebrisState) -> Vec<Dressed> {
    let Some(file) = library.model(debris.number) else { return Vec::new() };
    let animations: Vec<(Rc<Look>, bool)> = debris.animations.iter().filter_map(|&(number, repeat)| library.animation(number).map(|look| (look, repeat))).collect();
    debris
        .set
        .pieces
        .iter()
        .map(|piece| {
            let mut model = file.model.clone();
            for (look, repeat) in &animations {
                let age = piece.age as f32;
                look.apply(&mut model, if *repeat && look.length > 0.0 { age % look.length } else { age });
            }
            let (place, turns, size) = piece.drawn();
            Dressed { file: file.clone(), model, matrix: math::matrix(place, turns, size) }
        })
        .collect()
}

fn draw_models(canvas: &mut Canvas, view: &View, models: &[Dressed]) {
    let items: Vec<Item> = models.iter().map(|m| Item { model: &m.model, textures: &m.file.textures, matrix: m.matrix, shade: Shade::Effect }).collect();
    draw(canvas, view, &items);
}

/// Draws the particles of the effects that a rule chooses.
fn draw_effects(canvas: &mut Canvas, view: &View, library: &mut Library, sd: Option<&SdCard>, run: &Run, chosen: impl Fn(&crate::sim::EffectState) -> bool) {
    let frame = run.state.frame;
    let found: Vec<_> = run.state.effects.values().filter(|e| chosen(e)).filter_map(|e| library.effect(sd, e.number).map(|file| (e, file))).collect();
    let items: Vec<Placed> = found
        .iter()
        .map(|(effect, file)| {
            let (origin, turn, scale) = effect.object.drawn();
            Placed { effect: &file.effect, looks: &file.looks, frame: (frame - effect.born) as f32, stop: effect.stopped.map(|stop| (stop - effect.born) as f32), origin, axes: sango_effect::turned_axes(turn), scale }
        })
        .collect();
    if items.is_empty() {
        return;
    }
    let (seen, _) = sango_effect::seen(&items, view, canvas.width as f32 / canvas.height as f32);
    sango_effect::mix(&seen, view, &mut canvas.color, Some(&canvas.depth), canvas.width, canvas.height);
}

/// A piece of the place at a frame, with the animation of its materials.
fn piece(place: &Place, frame: i32) -> Model {
    let mut model = place.model.clone();
    if place.look.length > 0.0 {
        place.look.apply(&mut model, frame.max(0) as f32 % place.look.length);
    }
    model
}

/// The picture of the stage of a run, `width` × `height` pixels.
pub fn picture(run: &Run, library: &mut Library, sd: Option<&SdCard>, surroundings: &Surroundings, width: usize, height: usize) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    let view = view_of(run);
    let frame = run.state.frame;
    let screen = run.state.screen;

    // The place of the battle: the sky, the ground and a plate under each Pokémon.
    if screen.place_shown {
        if surroundings.open {
            canvas.gradient(SKY[0], SKY[1]);
        }
        let mut file = |number: usize| (number != 0).then(|| library.place(sd, number)).flatten();
        let (far, ground, plate) = (file(surroundings.far), file(surroundings.ground), file(surroundings.plate));
        if let Some(far) = far {
            let model = piece(&far, frame);
            draw(&mut canvas, &view, &[Item { model: &model, textures: &far.textures, matrix: Affine::IDENTITY, shade: Shade::Place }]);
            // The far side is behind everything.
            canvas.forget_depth();
        }
        if let Some(ground) = ground {
            let model = piece(&ground, frame);
            let matrix = math::matrix([0.0, GROUND, 0.0], [0.0; 3], [1.0; 3]);
            draw(&mut canvas, &view, &[Item { model: &model, textures: &ground.textures, matrix, shade: Shade::Place }]);
        }
        if let Some(plate) = plate.filter(|_| screen.plates_shown) {
            let model = piece(&plate, frame);
            let items: Vec<Item> = (0..2)
                .map(|who| {
                    let size = [0.715, 1.0, 1.15][run.cast[who].size.min(2)];
                    let distance = [200.0, 220.0, 250.0][run.cast[who].size.min(2)];
                    let z = if Run::is_near(run.setup, who) { distance } else { -distance };
                    Item { model: &model, textures: &plate.textures, matrix: math::matrix([0.0, 0.0, z], [0.0; 3], [size, 1.0, size]), shade: Shade::Place }
                })
                .collect();
            draw(&mut canvas, &view, &items);
        }
    }
    if screen.fill_on {
        canvas.cover([screen.fill[0], screen.fill[1], screen.fill[2]], screen.fill[3]);
    }

    let models: Vec<(Layer, bool, Dressed)> = run.state.models.values().filter(|m| m.visible).filter_map(|m| dress(library, m).map(|d| (m.layer, m.on_screen, d))).collect();
    let mut layers: [Vec<Dressed>; 4] = Default::default();
    for (layer, on_screen, model) in models {
        let place = match layer {
            _ if on_screen => 3,
            Layer::BeforePokemon => 0,
            Layer::AfterPokemon => 1,
            Layer::WithEffects => 2,
        };
        layers[place].push(model);
    }

    for debris in run.state.debris.values() {
        let place = match debris.layer {
            Layer::BeforePokemon => 0,
            Layer::AfterPokemon => 1,
            Layer::WithEffects => 2,
        };
        layers[place].extend(copies(library, debris));
    }

    // Behind the Pokémon.
    draw_models(&mut canvas, &view, &layers[0]);
    draw_effects(&mut canvas, &view, library, sd, run, |e| e.behind && !e.on_screen);

    // The Pokémon and their shadows.
    let shown: Vec<usize> = (0..2).filter(|&who| run.state.pokemon[who].visible && run.state.pokemon[who].charging_visible).collect();
    for &who in &shown {
        let state = &run.state.pokemon[who];
        if state.shadow && screen.place_shown {
            let asset = &run.cast[who];
            let place: V3 = math::add(state.object.position, state.object.offset);
            let radius = (asset.top * asset.model_scale() * state.object.scale[0].abs() * 0.4).clamp(8.0, 90.0);
            canvas.shadow(&view, [place[0], 0.0, place[2]], radius, SHADOW);
        }
    }
    let dressed: Vec<(usize, Model)> = shown
        .iter()
        .map(|&who| {
            let asset = &run.cast[who];
            let state = &run.state.pokemon[who];
            let motion = state.motion;
            let moved = asset.motions.as_ref().and_then(|pack| {
                let pose = pack.motion(motion.index).ok()?.pose(pack.joints.len(), motion.frame);
                Some(posed(&asset.model, pack, &pose))
            });
            let mut model = moved.unwrap_or_else(|| asset.model.clone());
            if let Some(look) = asset.looks.get(motion.index) {
                look.apply(&mut model, motion.frame);
            }
            if asset.endless.length > 0.0 {
                asset.endless.apply(&mut model, state.clock % asset.endless.length);
            }
            (who, model)
        })
        .collect();
    let items: Vec<Item> = dressed
        .iter()
        .map(|(who, model)| {
            let state = &run.state.pokemon[*who];
            Item { model, textures: &run.cast[*who].textures, matrix: run.pokemon_matrix(*who), shade: Shade::Pokemon { tint: state.tint, strength: state.tint_strength } }
        })
        .collect();
    draw(&mut canvas, &view, &items);

    // In front of the Pokémon, then the particles.
    draw_models(&mut canvas, &view, &layers[1]);
    draw_models(&mut canvas, &view, &layers[2]);
    draw_effects(&mut canvas, &view, library, sd, run, |e| !e.behind && !e.on_screen);

    // What stays in front of the camera.
    let close = screen_view();
    if !layers[3].is_empty() || run.state.effects.values().any(|e| e.on_screen) {
        canvas.forget_depth();
        draw_models(&mut canvas, &close, &layers[3]);
        draw_effects(&mut canvas, &close, library, sd, run, |e| e.on_screen);
    }
    canvas.cover([screen.fade[0], screen.fade[1], screen.fade[2]], screen.fade[3]);
    canvas
}
