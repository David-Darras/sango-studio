//! The whole conversion: a glTF file and the templates of the game become the files of a Pokémon model.

use crate::geometry::{self, Prim};
use crate::gltf::Source;
use crate::model::{self, Layout, MaterialSpec, MeshSpec, ModelSpec, Unit};
use crate::skeleton;
use crate::template::Template;
use crate::texture::{self, Texture};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The units of a model in the file: a unit of the game is a meter for 100 units of the model.
pub const UNITS_PER_METER: f64 = 100.0;
/// A texture is at most 256 x 256 pixels.
const TEXTURE_PIXELS: usize = 256 * 256;

/// What the conversion needs from the game: the models that the materials are cloned from, the shadow model that gives the layout of the
/// shadow, and the motion packs whose slots the new ones follow.
pub struct Templates {
    /// The model of the species that lends its slot, then the models that widen the choice of materials.
    pub models: Vec<Template>,
    pub shadow: Template,
    pub battle_motions: Vec<u8>,
    pub amie_motions: Vec<u8>,
}

/// The species whose models widen the choice of materials, after the species that lends its slot.
pub const POOL: [u32; 6] = [25, 494, 19, 51, 88, 26];

impl Templates {
    /// From the first pack of the model of the species that lends its slot, the first packs of the models of the pool, and the battle and
    /// Pokémon-Amie motion packs of the first species. A first pack holds the model and the shadow model.
    pub fn open(own: &[u8], pool: &[Vec<u8>], battle: &[u8], amie: &[u8]) -> Result<Templates, String> {
        let entries = crate::pc::entries(own)?;
        let [model, shadow] = [entries.first(), entries.get(1)].map(|e| e.ok_or("The pack of the model has no shadow model."));
        let mut models = vec![Template::open(model?)?];
        for pack in pool {
            models.push(Template::open(crate::pc::entries(pack)?.first().ok_or("A pack of the pool is empty.")?)?);
        }
        let first = |pack: &[u8]| -> Result<Vec<u8>, String> { Ok(crate::pc::entries(pack)?.first().ok_or("A motion pack is empty.")?.to_vec()) };
        Ok(Templates { models, shadow: Template::open(shadow?)?, battle_motions: first(battle)?, amie_motions: first(amie)? })
    }

    /// From the folder of extracted models of the tools of the repository (`pm0025_00_oras` and the like).
    pub fn from_tools(folder: &std::path::Path, species: u32) -> Result<Templates, String> {
        let read = |sp: u32, file: &str| std::fs::read(folder.join(format!("pm{sp:04}_00_oras")).join(file)).map_err(|e| format!("{file}: {e}"));
        let pool: Vec<Vec<u8>> = POOL.iter().map(|&sp| read(sp, "0_common.pc")).collect::<Result<_, _>>()?;
        Templates::open(&read(species, "0_common.pc")?, &pool, &read(species, "5_battle.pc")?, &read(species, "6_amie.pc")?)
    }
}

/// The files that come out of the conversion.
#[derive(Clone, Debug)]
pub struct Converted {
    pub model: Vec<u8>,
    pub shadow: Vec<u8>,
    pub textures: Vec<u8>,
    pub shiny_textures: Vec<u8>,
    pub battle_motions: Vec<u8>,
    pub amie_motions: Vec<u8>,
    pub material_names: Vec<String>,
    pub texture_names: Vec<String>,
}

/// A texture for the game from a picture: sides that are powers of two, at most 256 x 256 pixels (or `pixels`), RGBA8 for a picture with
/// transparency or RGB8, tiled.
pub fn make_texture(name: &str, rgba: &[u8], width: usize, height: usize, alpha: bool, pixels: Option<usize>) -> Texture {
    let (mut w, mut h) = (width, height);
    let limit = pixels.unwrap_or(TEXTURE_PIXELS);
    while w * h > limit && w.min(h) > 8 {
        w = (w / 2).max(8);
        h = (h / 2).max(8);
    }
    // The tiles of the GPU are 8 x 8: a flat picture of 2 x 2 is stretched.
    w = w.max(8);
    h = h.max(8);
    let source: Vec<u8> = if (w, h) == (width, height) {
        rgba.to_vec()
    } else {
        let image = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec()).expect("a picture of the right size");
        image::imageops::resize(&image, w as u32, h as u32, image::imageops::FilterType::Lanczos3).into_raw()
    };
    let bpp = if alpha { 4 } else { 3 };
    // The pixels as the game reads them: ABGR (with alpha) or BGR, column by column ([x][y]), then tiled.
    let mut columns = Vec::with_capacity(w * h * bpp);
    for x in 0..w {
        for y in 0..h {
            let p = &source[(y * w + x) * 4..(y * w + x) * 4 + 4];
            if alpha {
                columns.extend_from_slice(&[p[3], p[2], p[1], p[0]]);
            } else {
                columns.extend_from_slice(&[p[2], p[1], p[0]]);
            }
        }
    }
    Texture { name: name.to_string(), width: w, height: h, format: if alpha { 0 } else { 1 }, mips: 1, pixels: texture::tile(&columns, h, w, bpp) }
}

fn material_layout(blend: bool, wrap: [u8; 2]) -> Layout {
    let unit = Unit { wrap: (wrap[0], wrap[1]), mag: 1, min: 3, lod: 0, border: None, mapping: 0 };
    Layout { units: BTreeMap::from([(0, unit)]), sources: [0.0; 4], priority: 0, layer: u32::from(blend) }
}

/// A motion pack from the animations of the file. The animations that Studio exports are named after the motion (`Battle · Idle A`) and go
/// to their slot; the animations of other files fill the battle slots with the one that looks like an idle. Without animation the model
/// keeps its rest pose in every motion.
fn motions(source: &Source, skeleton: &skeleton::Skeleton, scale: f64, battle: bool, template: &[u8]) -> Result<Vec<u8>, String> {
    use crate::animate::{animations, build_pack, motion_index, motion_name, Animation, Slots};
    let names: HashMap<usize, String> = skeleton.index.iter().map(|(&k, &place)| (k, skeleton.joints[place].name.clone())).collect();
    let all = animations(source, &names, scale);
    let own = |a: &Animation| a.name.starts_with("Battle · ") || a.name.starts_with("Pokémon-Amie · ");
    let wanted: Vec<Animation> = if battle {
        all.iter().filter(|a| !a.name.starts_with("Pokémon-Amie · ")).cloned().collect()
    } else {
        all.iter().filter(|a| a.name.starts_with("Pokémon-Amie · ")).cloned().collect()
    };
    let table: HashMap<usize, String> = wanted.iter().filter(|a| own(a)).filter_map(|a| Some((motion_index(battle, &a.name)?, a.name.clone()))).collect();
    let idle = motion_name(battle, 0);
    // Without Pokémon-Amie motions the first slot is still and so are the others.
    build_pack(template, &skeleton.joints, &wanted, &Slots { table: &table, idle: &idle, still_after_first: false })
}

/// Makes the files of a model from a glTF file.
///
/// `model_name` is the name of the model in the files (for example `pm0025_01`), `scale_factor` makes the model bigger or smaller.
pub fn convert(source: &Source, templates: &Templates, model_name: &str, scale_factor: f64) -> Result<Converted, String> {
    convert_with(source, templates, model_name, scale_factor, false)
}

/// The same, and `flip_pictures` turns the pictures upside down first: the texture coordinates of a glTF file start at the top of the picture,
/// those of the game at the bottom of the picture as the viewer of Studio shows it.
pub fn convert_with(source: &Source, templates: &Templates, model_name: &str, scale_factor: f64, flip_pictures: bool) -> Result<Converted, String> {
    let scale = UNITS_PER_METER * scale_factor;
    // The joints that some vertex is weighted to.
    let mut used = BTreeSet::new();
    for primitive in &source.primitives {
        for (joints, weights) in primitive.joints.iter().zip(&primitive.weights) {
            for (&joint, &weight) in joints.iter().zip(weights) {
                if weight > 0.0 {
                    used.insert(joint as usize);
                }
            }
        }
    }
    let skeleton = skeleton::build(source, &used, scale as f32, model_name)?;

    // The materials and the textures: one texture for each distinct picture (several materials may share one), because the game loads all
    // the textures of a model at once.
    struct Info {
        name: String,
        blend: bool,
        additive: bool,
        wrap: [u8; 2],
        image: (usize, usize, Vec<u8>),
    }
    let mut infos = Vec::new();
    for material in &source.materials {
        let mut blend = material.blend;
        // Flames and glows are colour on black: they add themselves to the scene.
        let lower = material.name.to_lowercase();
        let additive = ["fire", "flame", "glow", "aura"].iter().any(|w| lower.contains(w));
        blend = blend || additive;
        let image = match material.image.and_then(|i| source.images.get(i)) {
            Some(image) => {
                // A picture with real transparency is blended whatever the material says.
                if image.rgba.chunks_exact(4).map(|p| p[3]).min().is_some_and(|alpha| alpha < 250) {
                    blend = true;
                }
                (image.width, image.height, image.rgba.clone())
            }
            None => {
                let c: Vec<u8> = material.base_color.iter().map(|v| (v * 255.0).round_ties_even() as u8).collect();
                (8, 8, c.iter().copied().cycle().take(8 * 8 * 4).collect())
            }
        };
        let image = if flip_pictures { (image.0, image.1, image.2.chunks(image.0 * 4).rev().flatten().copied().collect()) } else { image };
        infos.push(Info { name: material.name.clone(), blend, additive, wrap: material.wrap, image });
    }
    let mut textures: Vec<Texture> = Vec::new();
    let mut groups: HashMap<(usize, usize, Vec<u8>, bool, bool), String> = HashMap::new();
    let mut materials: Vec<MaterialSpec> = Vec::new();
    for info in &infos {
        let key = (info.image.0, info.image.1, info.image.2.clone(), info.blend, info.additive);
        let texture_name = match groups.get(&key) {
            Some(found) => found.clone(),
            None => {
                let name = format!("{model_name}_{}", info.name);
                // An eye is a few pixels of the screen: 128 x 128 is plenty.
                let small = info.name.contains("eye").then_some(128 * 128);
                textures.push(make_texture(&name, &info.image.2, info.image.0, info.image.1, info.blend && !info.additive, small));
                groups.insert(key, name.clone());
                name
            }
        };
        materials.push(MaterialSpec {
            name: info.name.clone(),
            slots: [Some(format!("{texture_name}.tga")), None, None],
            uv: [Some([1.0, 1.0, 0.0, 0.0, 0.0]), None, None],
            blend: vec![info.blend, info.blend, info.additive],
            layout: material_layout(info.blend, info.wrap),
        });
    }
    textures.push(make_texture("DummyTex", &[255; 32 * 32 * 4], 32, 32, true, None));
    let texture_names: HashMap<String, String> = textures.iter().map(|t| (format!("{}.tga", t.name), t.name.clone())).collect();

    // The meshes.
    let reference_sign = 1.0;
    let mut meshes: Vec<MeshSpec> = Vec::new();
    let mut all: Vec<(Prim, String, bool)> = Vec::new();
    for (name, places) in &source.meshes {
        let (mut faces, mut datas) = (Vec::new(), Vec::new());
        for &place in places {
            let primitive = &source.primitives[place];
            let material = materials.get(primitive.material).ok_or("A mesh uses a material that is not in the file.")?;
            let data = geometry::read_primitive(primitive, &skeleton.index, scale);
            let flip = geometry::winding_sign(&data) != reference_sign;
            faces.extend(geometry::build_faces(&data, &material.name, flip));
            all.push((data.clone(), material.name.clone(), flip));
            datas.push(data);
        }
        let refs: Vec<&Prim> = datas.iter().collect();
        meshes.push(MeshSpec { name: name.clone(), aabb: geometry::bounds(&refs), faces });
    }
    let all_refs: Vec<&Prim> = all.iter().map(|(p, _, _)| p).collect();
    let aabb = geometry::bounds(&all_refs);
    let spec = ModelSpec { aabb, joints: skeleton.joints.clone(), materials: materials.clone(), meshes };
    let model = model::write_model_file(&spec, &templates.models, model_name, &texture_names)?;

    // The shadow: the body, coarsened, on one material.
    let size = (0..3).map(|k| aabb.1[k] - aabb.0[k]).fold(f64::MIN, f64::max);
    let blends: HashMap<&str, bool> = materials.iter().map(|m| (m.name.as_str(), m.blend[0])).collect();
    let mut shadow_faces = Vec::new();
    for (data, material, flip) in &all {
        if blends.get(material.as_str()).copied().unwrap_or(false) {
            continue;
        }
        let coarse = geometry::cluster(data, size / geometry::SHADOW_DIVISIONS);
        if !coarse.tris.is_empty() {
            shadow_faces.extend(geometry::build_faces(&coarse, "PressShadowMaterial_PressShadow", *flip));
        }
    }
    let shadow_material = MaterialSpec {
        name: "PressShadowMaterial_PressShadow".into(),
        slots: [None, None, None],
        uv: [None, None, None],
        blend: vec![true, false],
        layout: Layout { units: BTreeMap::new(), sources: [0.0; 4], priority: 0, layer: 0 },
    };
    let shadow_spec = ModelSpec { aabb, joints: skeleton.joints.clone(), materials: vec![shadow_material], meshes: vec![MeshSpec { name: "BodySkin".into(), aabb, faces: shadow_faces }] };
    let shadow = model::write_model_file(&shadow_spec, std::slice::from_ref(&templates.shadow), &format!("{model_name}_shadow"), &texture_names)?;

    let textures_file = texture::write_texture_bch(textures.clone());
    Ok(Converted {
        model,
        shadow,
        shiny_textures: textures_file.clone(),
        textures: textures_file,
        battle_motions: motions(source, &skeleton, scale, true, &templates.battle_motions)?,
        amie_motions: motions(source, &skeleton, scale, false, &templates.amie_motions)?,
        material_names: materials.iter().map(|m| m.name.clone()).collect(),
        texture_names: textures.iter().map(|t| t.name.clone()).collect(),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;

    const TOOLS: &str = r"C:\Users\David\CLionProjects\ctrpluginframework\sango_plugin\private\tools";

    fn read(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn templates() -> Option<Templates> {
        let tools = Path::new(TOOLS);
        tools.exists().then(|| Templates::from_tools(tools, 25).unwrap())
    }

    fn first_difference(a: &[u8], b: &[u8]) -> String {
        match a.iter().zip(b).position(|(x, y)| x != y) {
            Some(at) => format!("first difference at {at:#x} ({:#x} against {:#x}), sizes {} and {}", a[at], b[at], a.len(), b.len()),
            None => format!("same start, sizes {} and {}", a.len(), b.len()),
        }
    }

    #[test]
    fn the_files_are_the_ones_of_the_python_tool() {
        let Some(templates) = templates() else { return };
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("model");
        let source = crate::gltf::read(&dir.join("body.gltf")).unwrap();
        let out = convert(&source, &templates, "pm0025_01", 1.0).unwrap();
        let work = dir.join("work").join("za_out_test");
        let mut wrong = Vec::new();
        for (name, bytes) in [("tex.bch", &out.textures), ("tex_rare.bch", &out.shiny_textures), ("battle.gmp", &out.battle_motions), ("amie.gmp", &out.amie_motions), ("shadow.bch", &out.shadow), ("model.bch", &out.model)] {
            let expected = read(&work.join(name));
            if &expected != bytes {
                wrong.push(format!("{name}: {}", first_difference(&expected, bytes)));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// The animated model: the motions are read back by the reader of Studio, with the number of frames of the animation (1.6 seconds).
    #[test]
    fn the_battle_motions_of_an_animated_model_are_read_back() {
        let Some(templates) = templates() else { return };
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("model");
        let source = crate::gltf::read(&dir.join("anim.gltf")).unwrap();
        let out = convert(&source, &templates, "pm0025_01", 1.0).unwrap();
        let pack = sango_core::motion::MotionPack::parse(&out.battle_motions).unwrap();
        let first = (0..pack.len()).find(|&i| pack.has(i)).expect("a motion");
        let motion = pack.motion(first).unwrap();
        assert_eq!(motion.frames, 48);
    }
}
