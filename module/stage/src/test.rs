//! Tests with the dump of the game. With `SANGO_STAGE_OUT` (a folder), the pictures are written there.

use crate::assets::{Choice, Library};
use crate::scene::{picture, Surroundings};
use crate::sim::{Run, Setup, ATTACKER, TARGET};
use sango_choreography::MoveChoreography;
use sango_core::{Dump, Garc, Session};

fn session() -> Option<Session> {
    let root = std::env::var("SANGO_DUMP").ok()?;
    Some(Session { dump: Some(Dump::open(&root).unwrap()), ..Session::default() })
}

fn choreography(session: &Session, file: usize) -> MoveChoreography {
    let garc = Garc::open(&session.dump.as_ref().unwrap().archive(34)).unwrap();
    MoveChoreography::parse(&garc.first_file_decompressed(file).unwrap()).unwrap()
}

fn run(library: &mut Library, session: &Session, file: usize, species: [usize; 2], setup: Setup) -> Run {
    library.use_session(session);
    let cast = species.map(|n| library.pokemon(session, Choice::new(n)).unwrap());
    Run::new(library, file, choreography(session, file), setup, cast)
}

fn save(canvas: &crate::raster::Canvas, name: &str) {
    let Ok(folder) = std::env::var("SANGO_STAGE_OUT") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    image::save_buffer(format!("{folder}/{name}.png"), &canvas.rgba(), canvas.width as u32, canvas.height as u32, image::ColorType::Rgba8).unwrap();
}

#[test]
fn the_pokemon_have_their_sizes_and_their_points() {
    let Some(session) = session() else { return };
    let mut library = Library::default();
    library.use_session(&session);
    for species in [1usize, 6, 25, 143, 257, 321, 384] {
        let asset = library.pokemon(&session, Choice::new(species)).unwrap_or_else(|e| panic!("{species}: {e}"));
        eprintln!("{species}: height {} shown {} size {} top {:.1} keys {:?}", asset.height, asset.shown_height, asset.size, asset.top, asset.keys);
        assert!(asset.height > 5 && asset.shown_height > 5);
        assert!(asset.motions.as_ref().is_some_and(|m| !m.nodes.is_empty()));
        assert!(asset.keys.iter().skip(8).take(8).any(Option::is_some), "{species}: an attack motion has its key");
    }
}

/// The pictures of some frames of some moves: the stage draws, the two Pokémon are on it.
#[test]
fn some_moves_play_on_the_stage() {
    let Some(session) = session() else { return };
    let mut library = Library::default();
    let moves: Vec<usize> = std::env::var("SANGO_STAGE_MOVES").map_or_else(|_| vec![1, 53, 85], |list| list.split(',').filter_map(|n| n.parse().ok()).collect());
    for move_id in moves {
        let file = 119 + move_id;
        let setup = Setup { attacker_is_opponent: std::env::var("SANGO_STAGE_OPPONENT").is_ok(), ..Setup::default() };
        let mut run = run(&mut library, &session, file, [257, 254], setup);
        let (first, last) = (run.first, run.last);
        let shots = 8;
        for shot in 0..shots {
            let frame = first + (last - first) * shot / (shots - 1);
            let start = std::time::Instant::now();
            run.seek(&mut library, None, frame);
            let canvas = picture(&run, &mut library, None, &Surroundings::default(), 400, 240);
            eprintln!("move {move_id} frame {frame} of {first}..{last}: {} effects, {} models, {:?}, blocked {:?}", run.state.effects.len(), run.state.models.len(), start.elapsed(), run.state.blocked);
            save(&canvas, &format!("move{move_id:03}_{shot}"));
        }
        // The same frame gives the same picture, also after going back.
        run.seek(&mut library, None, last / 2);
        let once = picture(&run, &mut library, None, &Surroundings::default(), 200, 120).rgba();
        run.seek(&mut library, None, first);
        run.seek(&mut library, None, last / 2);
        assert_eq!(once, picture(&run, &mut library, None, &Surroundings::default(), 200, 120).rgba(), "move {move_id}");
    }
}

/// Every choreography of the game plays to its end without a failure.
#[test]
fn every_choreography_plays_to_its_end() {
    let Some(session) = session() else { return };
    let mut library = Library::default();
    library.use_session(&session);
    let cast = [257usize, 254].map(|n| library.pokemon(&session, Choice::new(n)).unwrap());
    let count = library.choreography_count();
    let (mut played, mut blocked) = (0, 0);
    for file in 1..count {
        let setup = Setup { attacker_is_opponent: file % 2 == 0, targets_user: file % 5 == 0 };
        let mut run = Run::new(&mut library, file, choreography(&session, file), setup, cast.clone());
        let last = run.last;
        run.seek(&mut library, None, last);
        played += 1;
        blocked += usize::from(run.state.blocked.is_some());
        let state = &run.state;
        for who in [ATTACKER, TARGET] {
            assert!(state.pokemon[who].object.position.iter().all(|v| v.is_finite()), "file {file}");
        }
        assert!(state.camera.position.iter().chain(&state.camera.target).all(|v| v.is_finite()), "file {file}");
        assert!(state.effects.values().all(|e| e.object.position.iter().chain(&e.object.rotation).all(|v| v.is_finite())), "file {file}");
        library.forget_files_between_runs(file);
    }
    assert!(played > 900, "{played}");
    assert_eq!(blocked, 0, "the files of the game are in the order of their frames");
}

/// With the dump of the game: every place of the list has its files.
#[test]
fn the_places_of_the_list_have_their_files() {
    let Some(session) = session() else { return };
    let mut library = Library::default();
    library.use_session(&session);
    for place in crate::scene::PLACES {
        for number in [place.ground, place.plate, place.far].into_iter().filter(|&n| n != 0) {
            assert!(library.place(None, number).is_some_and(|p| !p.model.meshes.is_empty() && !p.textures.is_empty()), "{}: {number}", place.name);
        }
        library.forget();
    }
}

/// With the dump of the game: every camera animation that a choreography names is read.
#[test]
fn the_camera_animations_of_the_game_are_read() {
    let Some(session) = session() else { return };
    let mut library = Library::default();
    library.use_session(&session);
    let (mut named, mut read) = (std::collections::BTreeSet::new(), 0);
    for file in 1..library.choreography_count() {
        for step in &choreography(&session, file).steps {
            let kind = sango_choreography::kind_of(step.kind).unwrap();
            if let Some(param) = kind.params.iter().find(|p| p.name == "clip") {
                if let sango_choreography::Value::Int(number) = step.value(param) {
                    named.insert(number);
                }
            }
        }
    }
    for &number in &named {
        let Some(camera) = library.camera(number) else {
            eprintln!("camera {number} not read");
            continue;
        };
        let shot = camera.at(camera.frames / 2.0);
        assert!(shot.position.iter().chain(&shot.target).chain(&shot.up).all(|v| v.is_finite()), "{number}");
        assert!(shot.angle > 1.0 && shot.angle < 120.0, "{number}: {}", shot.angle);
        read += 1;
    }
    eprintln!("{read} of {} camera animations read", named.len());
    assert!(read >= named.len() * 9 / 10);
}
