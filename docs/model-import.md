# Model import: plan and state

Goal: the user gives Studio a `.glb` file (the layout that the Model tab exports) and Studio makes the 8 files of a Pokémon model.
A model without an animation stays in its T-pose in the game and in the view. The importer is written in Rust in the crate
`module/modelport`, so Studio needs no Python and no private file. The tool that does the same work today is in Python
(`private/tools`: `glb_port.py`, `gfbmdl_to_bch.py`, `gfbmot_to_gmp.py`). The Python tool is the **oracle**: each step of the Rust
code is checked against the files that the Python code writes (golden files in `module/modelport/tests/golden`) or against the files
of the game.

## The 8 files of a model

| Offset | Content | Where it comes from |
|---|---|---|
| +0 | pack: the model (BCH) and the shadow model (BCH) | made from the `.glb` |
| +1 | facial animations (eyes, mouth), by material name | copied from the template model |
| +2 | pack: the textures (BCH) | made from the pictures |
| +3 | pack: the shiny textures (BCH) | made from the shiny pictures, else the textures |
| +4 | pack: the setting of the portrait (the "ID" model) | copied from the template model |
| +5 | pack: the battle motions (GMP) and their visibility animations | the rest pose, or the animations of the `.glb` |
| +6 | pack: the Pokémon-Amie motions (GMP) | the rest pose, or the animations of the `.glb` |
| +7 | pack: the settings (size, camera) | made from the size of the model |

The **template model** is a model of the game (a species chosen by the user, Pikachu by default). Its materials, its command lists
and its metadata are cloned, because the materials of the game are made of GPU command lists that are not made from nothing.

## Steps

| Step | Module | State |
|---|---|---|
| PC packs | `pc.rs` | done: the packs of the game are made again byte for byte |
| Patricia dictionaries | `patricia.rs` | done: all the dictionaries of 144 models of the game are built again node for node |
| BCH writer (sections, strings, dictionaries, relocations) | `bch.rs` | done: the file of the textures is byte for byte the file of the Python tool |
| Textures: tiles, sizes, file of the textures | `texture.rs`, `convert.rs` | done (same golden file); the conversion from a picture is in `convert::make_texture` |
| glTF reader: skeleton, meshes, weights, materials, pictures, animations | `gltf.rs` | done: reads a `.glb` or `.gltf` (the layout of the Model tab export); tested on a small file written by the test |
| Geometry: tangents, groups of at most 20 bones, vertex buffers of 56 bytes, shadow model | `geometry.rs` | done: joints, vertices and draws equal the Python tool on a test model |
| Model writer from a template (`write_model_bch`) | `model.rs`, `template.rs` | done: model, shadow and textures are byte for byte the files of the Python tool (test model, template Pikachu) |
| Motions: rest pose (T-pose) | `motion.rs` | done: a model without animation stays in its rest pose; the animations of the file are sampled at 30 frames a second and written as curves with slopes (kind 7, `animate.rs`): the motions of Venusaur, Charizard, Mewtwo and Pikachu exported and imported again stay within 0.01 of the original rotations. The Python tool used flat slopes (kind 6) with few keys, which looks jerky. A clip named like the export (`Battle · Idle A`, `Pokémon-Amie · Happy A`) goes to its slot, other clips fill the battle slots (the idle, else the clip with "wait" in its name, else the first). Rotation and translation only: the scale is not animated. From a glTF file the pictures are turned upside down first (`convert_with`) and the repeat mode of the sampler (mirrored or repeat) is kept. A model taller than 30 units is taken as being in the units of the game (what the Model tab exports), else the file is in metres |
| The 8 files | `files.rs` | done: the 8 packs are byte for byte the ones of the Python tool |
| Studio: buttons Import model and Back to the model of the pack | `module/species/src/model_import.rs`, `module/pack` (`Library::set_model`) | done for the species and the forms of the packs; tested with the dump (the Model tab reads the imported model back). Not tested in the game |
| Replace a model of a species of the game | `Library::replace_game_model`, pack `studio-models` | done: the list of the species is moved to the end of the SD files with its models copied, and the imported files overwrite the slot (no plugin change). Tested with the dump; not in the game |

## The model writer

The Python function `write_model_bch` (about 400 lines) and its helpers (about 700 lines) do this: read a template BCH (the structures
of its model, its meshes, its materials, its metadata); write the dictionaries and the model structure; write the bones (flags, parent,
scale, rotation, translation, inverse matrix); write one mesh for each group of at most 20 bones with the command lists of the template
mesh (the draw count and the attribute setup are changed); clone, for each material of the source, the template material that feeds the
same texture units (the combiner of a material is set for exactly these units); write the lookup tables (LUT) that the cloned materials
read; and write the metadata with the bounding box. The choice of the template material is empirical (names, units, blending): the
rules are in `pick` of the Python file.

## How to check a step

1. Write the Python function call that makes the reference file in `module/modelport/tests/golden` (a small file).
2. Make the Rust function give the same bytes. The test compares the bytes.
3. For a step that reads a file of the game, the test reads the dump (`SANGO_DUMP`) and checks that the Rust code gives back what the game has.
