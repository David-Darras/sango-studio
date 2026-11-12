//! Prints the bands of the meshes of a model of the dump and writes its
//! textures as PNG files. A temporary probe: do not commit.

use std::collections::HashMap;

fn main() {
    let root = std::env::var("SANGO_DUMP").expect("set SANGO_DUMP");
    let out = std::env::var("SANGO_RENDER_OUT").expect("set SANGO_RENDER_OUT");
    let species: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let dump = sango_core::Dump::open(&root).unwrap();
    let garc = sango_core::Garc::open(&dump.archive(8)).unwrap();
    let first = 3 + 8 * (species - 1);
    let parts = |file: usize| -> Vec<Vec<u8>> {
        let bytes = sango_core::lz::decompress(garc.first_file(file).unwrap()).unwrap();
        let bundle = sango_core::Bundle::parse(bytes).unwrap();
        (0..bundle.resource_count()).map(|i| bundle.resource(i).unwrap().to_vec()).collect()
    };
    let model = sango_core::model::parse_model(&parts(first)[0]).unwrap();
    let textures = sango_core::model::parse_textures(&parts(first + 2)[0]).unwrap();
    for (k, mesh) in model.meshes.iter().enumerate() {
        let material = &model.materials[mesh.material];
        let place = |uv: [f32; 2]| {
            (material.texture_scale[0] * (uv[0] - material.texture_translation[0]), material.texture_scale[1] * (uv[1] - material.texture_translation[1]))
        };
        let mut low = (f32::MAX, f32::MAX);
        let mut high = (f32::MIN, f32::MIN);
        for vertex in &mesh.vertices {
            let p = place(vertex.uv);
            low = (low.0.min(p.0), low.1.min(p.1));
            high = (high.0.max(p.0), high.1.max(p.1));
        }
        println!(
            "mesh {k} material {} ({:?}, texture {:?}, wrap {:?}, transparent {}) uv' u [{:.3}, {:.3}] v [{:.3}, {:.3}]",
            mesh.material, material.name, material.texture, material.texture_wrap, material.transparent, low.0, high.0, low.1, high.1
        );
    }
    let _ = HashMap::<(), ()>::new();
    for texture in &textures {
        image::save_buffer(format!("{out}/tex_{}.png", texture.name), &texture.rgba, texture.width as u32, texture.height as u32, image::ColorType::Rgba8).unwrap();
    }
}
