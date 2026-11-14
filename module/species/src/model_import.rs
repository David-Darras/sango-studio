//! The import of a 3D model (glTF or GLB) for a species or a form of a pack, and the return to the model that the pack shipped.
//!
//! The work runs in a thread: the conversion and the writing of all the model files of the packs take some seconds.

use sango_core::{lz, Dump, Garc, SdCard};
use sango_modelport::convert::{Templates, POOL};
use sango_modelport::files::TemplatePacks;
use sango_pack::{Library, Ref};
use std::path::PathBuf;

/// The species whose packs the imported models follow: Pikachu.
const TEMPLATE: u32 = 25;

/// What the user asked for.
pub enum Request {
    /// Replaces the model by the one in a glTF file.
    Import { file: PathBuf, species: Ref, form: u16, model_name: String, scale: f64 },
    /// Puts the model that the pack shipped back.
    Reset { species: Ref, form: u16 },
    /// Replaces a model of a species of the game (`slot` 0 is the normal model).
    ImportGame { file: PathBuf, species: u16, slot: u8, model_name: String, scale: f64 },
    /// Puts the model of the game back.
    ResetGame { species: u16, slot: u8 },
}

/// A file of the model archive, decompressed.
fn game_file(archive: &Garc, id: usize) -> Result<Vec<u8>, String> {
    let bytes = archive.first_file(id).ok_or_else(|| format!("The file {id} of the model archive is missing."))?;
    if lz::is_compressed(bytes) {
        lz::decompress(bytes)
    } else {
        Ok(bytes.to_vec())
    }
}

/// The first file of the first model of a species of the game: the list of the models says where it is.
fn first_file(archive: &Garc, species: u32) -> Result<usize, String> {
    let list = archive.first_file(0).ok_or("The list of the models is missing.")?;
    let at = (species as usize - 1) * 4;
    let entry = list.get(at..at + 4).ok_or("The list of the models is cut.")?;
    Ok(u16::from_le_bytes([entry[0], entry[1]]) as usize * 8 + 3)
}

/// The templates that the importer clones the materials and the packs from, read from the archive of the game.
fn templates(archive: &Garc) -> Result<(Templates, TemplatePacks), String> {
    let base = first_file(archive, TEMPLATE)?;
    let pool: Vec<Vec<u8>> = POOL.iter().map(|&species| game_file(archive, first_file(archive, species)?)).collect::<Result<_, _>>()?;
    let templates = Templates::open(&game_file(archive, base)?, &pool, &game_file(archive, base + 5)?, &game_file(archive, base + 6)?)?;
    let packs = TemplatePacks { face: game_file(archive, base + 1)?, battle: game_file(archive, base + 5)?, amie: game_file(archive, base + 6)?, setting: game_file(archive, base + 7)? };
    Ok((templates, packs))
}

/// Does the work and writes everything on the SD card (the pack, the registry, the table and the model files of the plugin).
pub fn run(sd_root: &str, dump_root: &str, request: &Request) -> Result<String, String> {
    let sd = SdCard::open(sd_root)?;
    let dump = Dump::open(dump_root)?;
    let mut library = Library::open(&sd)?;
    let message = match request {
        Request::Import { file, model_name, scale, .. } | Request::ImportGame { file, model_name, scale, .. } => {
            let source = sango_modelport::gltf::read(file)?;
            let archive = Garc::open(&dump.archive(8))?;
            let (templates, packs) = templates(&archive)?;
            let files = sango_modelport::files::model_files(&source, &templates, &packs, model_name, *scale)?;
            match request {
                Request::Import { species, form, .. } => library.set_model(&sd, species, *form, Some(files))?,
                Request::ImportGame { species, slot, .. } => library.replace_game_model(&sd, *species, *slot, Some(files))?,
                _ => unreachable!(),
            }
            format!("Imported {}. Restart the game to see it.", file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
        }
        Request::Reset { species, form } => {
            library.set_model(&sd, species, *form, None)?;
            "The model of the pack is back. Restart the game to see it.".to_string()
        }
        Request::ResetGame { species, slot } => {
            library.replace_game_model(&sd, *species, *slot, None)?;
            "The model of the game is back. Restart the game to see it.".to_string()
        }
    };
    library.apply(&sd, &dump)?;
    Ok(message)
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_pack::pack::{self, ModelRef, Pack, PackSpecies};

    struct VecFiles(Vec<Vec<u8>>);

    impl crate::model_tab::Files for VecFiles {
        fn get(&self, id: usize) -> Option<std::borrow::Cow<'_, [u8]>> {
            self.0.get(id).map(|f| std::borrow::Cow::Borrowed(f.as_slice()))
        }
    }

    fn rotation_matrix(e: [f32; 3]) -> [[f32; 3]; 3] {
        let (cx, sx, cy, sy, cz, sz) = (e[0].cos(), e[0].sin(), e[1].cos(), e[1].sin(), e[2].cos(), e[2].sin());
        let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
        let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
        let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
        let mul = |a: [[f32; 3]; 3], b: [[f32; 3]; 3]| -> [[f32; 3]; 3] { std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum())) };
        mul(rz, mul(ry, rx))
    }

    /// With the dump of the game (`SANGO_DUMP`): the motion of a model that is exported, imported and read again moves like the original.
    #[test]
    fn an_exported_motion_moves_like_the_original_after_the_import() {
        let Ok(dump_root) = std::env::var("SANGO_DUMP") else { return };
        let species: usize = std::env::var("SANGO_MOTION_SPECIES").ok().and_then(|s| s.parse().ok()).unwrap_or(25);
        let dump = Dump::open(&dump_root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let glb = crate::model_tab::glb_of_game_model(&archive, species).unwrap();
        let source = sango_modelport::gltf::read_bytes(&glb, None).unwrap();
        let (templates, packs) = templates(&archive).unwrap();
        let files = VecFiles(sango_modelport::files::model_files(&source, &templates, &packs, "pm0724_00", 1.0).unwrap());
        let original_first = first_file(&archive, species as u32).unwrap();
        let original = crate::model_tab::battle_motions_for_test(&archive, original_first).unwrap();
        let converted = crate::model_tab::battle_motions_for_test(&files, 0).unwrap();
        let index = (0..original.len()).find(|&i| original.has(i) && converted.has(i)).unwrap();
        let (a, b) = (original.motion(index).unwrap(), converted.motion(index).unwrap());
        let frames = a.frames.min(b.frames) as usize;
        let mut worst_angle = 0.0f32;
        let mut worst_move = 0.0f32;
        let mut compared = 0;
        for frame in (0..frames).step_by(2) {
            let (pa, pb) = (a.pose(original.joints.len(), frame as f32), b.pose(converted.joints.len(), frame as f32));
            for (ja, joint) in original.joints.iter().enumerate().skip(1) {
                let Some(jb) = converted.joints.iter().position(|j| j.name == joint.name) else { continue };
                let (Some(ra), Some(rb)) = (pa.joints[ja].rotation, pb.joints[jb].rotation) else { continue };
                // Two sets of angles can turn the same way: the matrices are compared.
                let (ma, mb) = (rotation_matrix(ra), rotation_matrix(rb));
                for r in 0..3 {
                    for c in 0..3 {
                        worst_angle = worst_angle.max((ma[r][c] - mb[r][c]).abs());
                    }
                }
                if let (Some(ta), Some(tb)) = (pa.joints[ja].translation, pb.joints[jb].translation) {
                    worst_move = worst_move.max((0..3).map(|k| (ta[k] - tb[k]).abs()).fold(0.0, f32::max));
                }
                compared += 1;
            }
        }
        eprintln!("compared {compared} joint poses: worst angle {worst_angle}, worst move {worst_move}");
        assert!(compared > 100);
        assert!(worst_angle < 0.06, "a rotation is off by {worst_angle}");
        assert!(worst_move < 0.6, "a translation is off by {worst_move}");
    }

    /// Debug helper: converts the .glb file `SANGO_GLB_IN` and draws the result with the viewer of the Model tab in `SANGO_GLB_RENDER`
    /// (a PNG file), and writes the pictures of the converted model next to it.
    #[test]
    fn a_converted_glb_file_is_drawn() {
        let (Ok(dump_root), Ok(input), Ok(output)) = (std::env::var("SANGO_DUMP"), std::env::var("SANGO_GLB_IN"), std::env::var("SANGO_GLB_RENDER")) else { return };
        let dump = Dump::open(&dump_root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let source = sango_modelport::gltf::read(std::path::Path::new(&input)).unwrap();
        let (templates, packs) = templates(&archive).unwrap();
        let files = sango_modelport::files::model_files(&source, &templates, &packs, "pm0724_00", 1.0).unwrap();
        // The settings (pack 7): the height in centimetres and the collision box must be those of a Pokémon, not of a giant.
        let setting = sango_modelport::pc::entries(&files[7]).unwrap()[0].to_vec();
        let height = i32::from_le_bytes(setting[..4].try_into().unwrap());
        eprintln!("height {height} cm");
        assert!((10..1000).contains(&height), "the model is {height} cm tall");
        let files = VecFiles(files);
        let (model, textures) = crate::model_tab::read_model_for_test(&files, 0, false).unwrap();
        let by_name: std::collections::HashMap<_, _> = textures.iter().map(|t| (t.name.clone(), t.clone())).collect();
        for (k, yaw) in [(0, 0.5f32), (1, 2.6)] {
            let mut camera = sango_core::render::Camera::default();
            camera.yaw = yaw;
            let pixels = sango_core::render::render(&model, &by_name, &camera, 440, 440);
            let image = image::RgbaImage::from_raw(440, 440, pixels).unwrap();
            image.save(format!("{output}_{k}.png")).unwrap();
        }
        for t in &textures {
            let image = image::RgbaImage::from_raw(t.width as u32, t.height as u32, t.rgba.clone()).unwrap();
            image.save(format!("{output}_tex_{}.png", t.name)).unwrap();
        }
    }

    /// With the dump of the game (`SANGO_DUMP`): the exported .glb file of a model of the game, with its motions, is imported again and the
    /// motions of the new model move.
    #[test]
    fn the_motions_of_an_exported_model_come_back_in_the_import() {
        let Ok(dump_root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&dump_root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let glb = crate::model_tab::glb_of_game_model(&archive, std::env::var("SANGO_GLB_SPECIES").ok().and_then(|s| s.parse().ok()).unwrap_or(25)).unwrap();
        if let Ok(path) = std::env::var("SANGO_GLB_OUT") {
            std::fs::write(path, &glb).unwrap();
        }
        let source = sango_modelport::gltf::read_bytes(&glb, None).unwrap();
        assert!(source.animations.len() > 10, "the export has the motions: {}", source.animations.len());
        let (templates, packs) = templates(&archive).unwrap();
        let files = sango_modelport::files::model_files(&source, &templates, &packs, "pm0724_00", 1.0).unwrap();
        // The battle pack: the first entry has the motions; the idle (slot 1) is not the rest pose of one frame.
        let battle = sango_modelport::pc::entries(&files[5]).unwrap()[0].to_vec();
        let count = u32::from_le_bytes(battle[..4].try_into().unwrap()) as usize;
        let starts: Vec<usize> = (0..=count).map(|i| u32::from_le_bytes(battle[4 + 4 * i..8 + 4 * i].try_into().unwrap()) as usize).collect();
        // The size of each motion: up to the next one that exists.
        let sizes: Vec<usize> = (1..count).filter(|&k| starts[k] != 0).map(|k| starts[k + 1..].iter().find(|&&n| n != 0).map_or(0, |n| n - starts[k])).collect();
        let moving = sizes.iter().filter(|&&size| size > 200);
        // Studio's own reader reads the motions back, with the frames of the original.
        let pack = sango_core::motion::MotionPack::parse(&battle).unwrap();
        let first = (0..pack.len()).find(|&i| pack.has(i)).unwrap();
        let motion = pack.motion(first).unwrap();
        assert!(motion.frames > 1, "a motion of {} frames", motion.frames);
        assert!(moving.count() >= 8, "most battle motions move");
    }

    /// With the dump of the game (`SANGO_DUMP`): a model made from a glTF file is read by the model tab like the models of the game.
    #[test]
    fn an_imported_model_is_read_back_like_a_model_of_the_game() {
        let Ok(dump_root) = std::env::var("SANGO_DUMP") else { return };
        let outer = std::env::temp_dir().join(format!("sango_import_{}", std::process::id()));
        let sdmc = outer.join("sdmc");
        std::fs::create_dir_all(sdmc.join("sango-studio")).unwrap();
        let sd = SdCard::open(sdmc.to_str().unwrap()).unwrap();
        let species = PackSpecies { name: "Alpha".into(), model: ModelRef::File { file: "Alpha".into() }, data: pack::to_hex(&[7u8; pack::DATA_SIZE]), moves: Vec::new(), evolutions: Vec::new() };
        let pack = Pack { id: "modeled".into(), name: "modeled".into(), version: "1".into(), species: vec![species], ..Pack::default() };
        let file = sdmc.join("modeled.sangopack");
        pack.save_with_models(&file, &mut |_| Ok((0..8u8).map(|n| vec![n; 16]).collect())).unwrap();
        let mut library = Library::default();
        library.install(Pack::open(&file).unwrap()).unwrap();
        let dump = Dump::open(&dump_root).unwrap();
        library.apply(&sd, &dump).unwrap();

        let glb = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("modelport").join("tests").join("golden").join("model").join("body.gltf");
        let request = Request::Import { file: glb, species: Ref::key("Alpha"), form: 0, model_name: "pm0724_00".into(), scale: 1.0 };
        let message = run(sdmc.join("sango-studio").to_str().unwrap(), &dump_root, &request).unwrap();
        assert!(message.starts_with("Imported"), "{message}");

        let models = crate::model_tab::SdModels::open(&sdmc.join("sango").join("pokemodel")).unwrap();
        let carrier = crate::model_tab::variants_of_sd(&models, 494);
        // The model of the pack that was replaced keeps its slot (the second one); the imported model has the next one.
        let own = carrier.first().expect("the carrier has its own model");
        let (model, textures) = crate::model_tab::read_model_for_test(&models, own.file + 16, false).unwrap();
        assert_eq!(model.bones.len(), 4, "the skeleton of the glTF file and the root of the game");
        assert!(!textures.is_empty());
        assert!(model.meshes.iter().map(|m| m.triangles.len()).sum::<usize>() >= 24, "the two boxes");

        // The model of a species of the game: Venusaur (3), normal model.
        let glb = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("modelport").join("tests").join("golden").join("model").join("body.gltf");
        let sd_root = sdmc.join("sango-studio");
        let request = Request::ImportGame { file: glb, species: 3, slot: 0, model_name: "pm0003_80".into(), scale: 1.0 };
        run(sd_root.to_str().unwrap(), &dump_root, &request).unwrap();
        // The index of the files has one entry for each file: a replaced file must not be there two times.
        let index = std::fs::read(sdmc.join("sango").join("pokemodel").join("index.bin")).unwrap();
        let count = u32::from_le_bytes(index[..4].try_into().unwrap()) as usize;
        let ids: Vec<u32> = (0..count).map(|i| u32::from_le_bytes(index[4 + 8 * i..8 + 8 * i].try_into().unwrap())).collect();
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(ids, unique, "the index lists a file two times");
        let models = crate::model_tab::SdModels::open(&sdmc.join("sango").join("pokemodel")).unwrap();
        let venusaur = crate::model_tab::variants_of_sd(&models, 3);
        assert!(venusaur.len() >= 2, "the species keeps all its models");
        let (replaced, _) = crate::model_tab::read_model_for_test(&models, venusaur[0].file, false).unwrap();
        assert_eq!(replaced.bones.len(), 4, "the normal model is the imported one");
        let (other, _) = crate::model_tab::read_model_for_test(&models, venusaur[1].file, false).unwrap();
        assert!(other.bones.len() > 4, "the other models are the ones of the game");
        run(sd_root.to_str().unwrap(), &dump_root, &Request::ResetGame { species: 3, slot: 0 }).unwrap();
        assert!(run(sd_root.to_str().unwrap(), &dump_root, &Request::ResetGame { species: 3, slot: 0 }).is_err());

        // The reset gives back the model of the pack.
        let back = run(sdmc.join("sango-studio").to_str().unwrap(), &dump_root, &Request::Reset { species: Ref::key("Alpha"), form: 0 }).unwrap();
        assert!(back.contains("back"), "{back}");
        let _ = std::fs::remove_dir_all(&outer);
    }
}
