# Species packs: design

Status:

- Done in Studio: the crate `module/pack` (format, checks, registry, `table.bin`), and the three first packs made
  from the data files of the plugin (`gen7`, `gen8`, `forms`: 81 + 216 species, 138 forms). A test builds the table
  from the packs and compares it, byte for byte, with the data files of the plugin.
- Done in the plugin: `species_packs.cc` reads `table.bin`; `species_table.cc`, `alolan_forms.cc`, the box filler and the checks use it. Compiles for ORAS and XY; not tested in game. The `.inc` files of the species are no longer used.
- Done: the page Packs (install, remove, install again, activate the models folder).
- Done: the models are inside the packs (8 files per model), and Studio writes the model files of the plugin.
- To do: the buttons in the Species page, the import of a `.glb` model.

## Goal

Sango Studio owns all the data of the species that the game does not have (new species, new forms,
Mega Evolutions, regional forms). The Sango plugin only loads this data from the SD card. The plugin
contains no data about these species.

A **pack** is one file that holds the species, the forms and the models of a set of Pokémon. A user
installs a pack with Sango Studio. Users share packs with each other.

## What is in the plugin today (to remove)

| File | Content |
|---|---|
| `lib/src/pokemon/data/gen7_species.inc` | 81 species of generation VII: name, data, learnset, evolutions |
| `lib/src/pokemon/data/gen8_species.inc` | 216 species of generation VIII and IX |
| `lib/src/pokemon/data/alolan_form.inc` | extra forms (Alolan, Galarian, new Megas): species data |
| `lib/src/pokemon/data/form.inc` | names of the forms for the menus |
| `lib/src/pokemon/data/evolve.inc`, `za_mega.inc` | evolutions, Mega triggers |
| `SpeciesTable` constants | `kGen7Count`, `kGen8Count`, `kAlolanCount`, carriers (494, 721) |

The models are already outside the plugin: `sdmc:/sango/pokemodel/<n>.pc`, listed by `index.bin`,
with the table of the model slots in `0.raw`.

## Pack format

A pack is a ZIP file with the extension `.sangopack`. It has `manifest.json` and the 3D models of its species and
forms: for each model, the 8 files of a model of the game, `models/<name>/0.pc` to `models/<name>/7.pc` (stored, not
compressed).

```json
{
  "id": "gen7", "name": "Generation VII species", "version": "1.0",
  "author": "...", "license": "...", "credit": "...",
  "species": [
    { "name": "Rowlet", "model": { "file": "Rowlet" }, "data": "<80 bytes, hexadecimal>",
      "moves": [{ "id": 33, "level": 1 }],
      "evolutions": [{ "method": 4, "arg": 17, "species": "Dartrix" }] }
  ],
  "forms": [
    { "species": 3, "form": 2, "name": "Mega", "data": "<80 bytes, hexadecimal>" }
  ]
}
```

Rules:

- The **name is the key** of a species. It has 1 to 23 characters and is not a number. Two species with the same
  name cannot be installed together.
- A species reference is a name (a species of a pack) or a number from 1 to 721 (a species of the game).
- A form belongs to a species of the game or to a species of a pack, in this pack or in another one that is installed.
- `model` names the 3D model: `{ "file": "<name>" }` is the folder `models/<name>` of the pack, `{ "native": n }` is the
  model of the species `n` of the game. A form without `model` shows the model that the game has for it. The name of a
  model is scoped to the pack: two packs can have a model with the same name. Species and forms of one pack that name
  the same model share it.
- A pack has at most 24 level-up moves and 4 evolutions for each species.
- `credit` is shown by Studio before the installation.
- Studio checks the pack before it installs it: sizes, names, references.

## Registry on the SD card

Studio keeps `sdmc:/sango-studio/species/registry.json`. It lists, in the order of installation:

- the installed packs (id, name, version, active or removed),
- the species: the name and the pack. The **number is 724 plus the position in the list**,
- the forms: the species, the form number and the pack. The position is the order in which the plugin adds them,
- the models: the name and the pack. The position gives the place of the model in the model slots.

A number never changes after installation, so a save file keeps working. A removed pack keeps its places: they
stay in the table as empty entries. Installing the pack again gives its numbers back. A pack that uses a species
of another pack needs that pack to be installed first.

## Files that the plugin reads

Studio writes these files at each installation. The plugin reads binary files only.

| File | Content |
|---|---|
| `species/table.bin` | the species (256 bytes each) and the forms (88 bytes each); the layout is in `module/pack/src/table.rs` |
| `../sango/pokemodel/0.raw`, `index.bin`, `<file>.pc` | the 3D models (see "Models") |
| `species/packs/<id>.sangopack` | a copy of each installed pack (for Studio only) |
| `species/registry.json` | the numbers (for Studio only) |

`reload.bin` tells the plugin that the table changed. A new species needs a restart of the game, because the
game builds its species table at start.

## Plugin changes

1. `SpeciesTable` reads `species/table.bin` instead of the `.inc` files (done, in `species_packs.cc`). The
   size of the table is fixed to the capacity above and filled at run time.
2. `AlolanForms`, `MegaEvolution`, the names of the forms and the evolutions read the same file.
3. The `.inc` files and the generator that writes them are removed from the plugin repository.
4. The `NewSpecies()` API of the products stays; it adds species after the species of the packs.

## Studio changes

1. New crate `module/pack`: read and write a pack, check it, allocate numbers, update the registry,
   generate the files above. Pure data, with tests on a dump.
2. New page **Packs**: installed packs, **Install…**, **Remove**, **Export…** (make a pack from
   chosen species), credit and licence of each pack.
3. Species page: the button **+ New species** and **+ New form** (same rule as for items: add with a
   button, edit with the usual tabs). Species and forms from packs show in the list with their
   number.
4. Converted models: Studio imports a finished `.pc`. The converters from other games stay outside
   Studio for now (Python tools).

## Capacity (decided 2026-11-13)

The plugin keeps **384 species** from the packs (the numbers 724 to 1107) and **256 forms** (384 table
slots, because a form of a species that already has forms copies them). That is enough for all the
species up to generation IX (about 304 species that ORAS lacks) and 64 species that the user creates.
The species that a product adds with `NewSpecies()` come after them, from the number 1108 (it was 1021).
The same numbers are in `module/pack/src/library.rs` and in `pokemon/patch/species_packs.h`.

## Models

The plugin reads the model of a Pokémon as 8 files of the archive 8. The file 0 of the archive is a list: for each
species, the first model slot, the number of slots and flags. A model of a pack uses a slot of a **carrier** species
(Victini 494, then Volcanion 721): the plugin changes the model request of a species of a pack into the carrier species
and a form of the carrier (`model_form`: 1 to 254 is Victini, 257 to 510 is Volcanion). Studio writes all of this when a
pack is installed or removed (`Library::write_models`):

- the models of the registry get the forms 1, 2, 3... in the order of the registry. The list of the models keeps the
  number of slots of a species in one byte, so a carrier has 255 slots at most: its own model and 254 models. The packs
  hold 508 models at most;
- the list (`0.raw`) is the list of the game, with a new block of slots for each carrier, after the last slot of the game.
  The slot 0 of the block is the own model of the carrier, copied from the archive of the dump; the next slots are the
  models of the packs;
- each file is `<file number>.pc`: the size (4 bytes) and the data. `index.bin` lists the files, sorted. The plugin has
  room for 6 144 files;
- the folder `sango/pokemodel` is made again each time. The models of a removed pack keep their slot and have no files.

This is why Studio needs the romfs dump to install a pack.

## Models and animations: import (next phase)

The Model tab already exports a `.glb` file (the model, the clips of the animations) and the pictures as PNG
files. The import must read **the same layout**: a user exports a model, changes it in a 3D program, and
imports it back; a pack made by someone else uses the same layout. The steps:

1. A user gives a `.glb` (and its PNG pictures) to a species or a form.
2. Studio converts it to the 8 files of a model of the game (`.bch` model, textures, motions). The converter exists today
   as Python tools (`glb_port.py` and the tools it uses); it must be written again in Rust inside Studio.
3. The 8 files go in the pack as `models/<name>/`, and Studio gives the model a slot as above.

## Migration of the existing data

1. A tool reads the `.inc` files and writes four packs: `gen7`, `gen8`, `regional-forms`,
   `z-a-megas`. The packs are the first content that users can install.
2. A test installs the four packs in an empty SD card and checks that the generated
   `table.bin` is byte-for-byte what the plugin builds today from the `.inc` files.
3. Only after this test, the `.inc` files leave the plugin.

## Open points

- **Memory of the plugin.** The table is about 132 KB and the file buffers about 120 KB, both static. The old
  `.inc` files, no longer used, took about 100 KB of code and data. Check the size of the plugin at the next build.
- **Model slots.** The carriers (Victini 494, Volcanion 721) have 255 slots each. Studio must allocate
  slots across all the packs. A third carrier may be needed.
- **Distribution.** Packs are files that users send to each other. A catalogue with download is
  possible later (a JSON index at an URL).
- **Third-party models.** Keep the credit and the licence of each model in the manifest. Studio
  shows them before the user installs a pack.
