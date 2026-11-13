# Species packs: design

Status:

- Done in Studio: the crate `module/pack` (format, checks, registry, `table.bin`), and the three first packs made
  from the data files of the plugin (`gen7`, `gen8`, `forms`: 81 + 216 species, 138 forms). A test builds the table
  from the packs and compares it, byte for byte, with the data files of the plugin.
- To do: the loader in the plugin (see "Plugin changes"), the page Packs, the buttons in the Species page, the
  models inside the packs.

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

A pack is a ZIP file with the extension `.sangopack`. Today it has one entry, `manifest.json`. The models
will be added as more entries (see "Open points").

```json
{
  "id": "gen7", "name": "Generation VII species", "version": "1.0",
  "author": "...", "license": "...", "credit": "...",
  "species": [
    { "name": "Rowlet", "model_form": 1, "data": "<80 bytes, hexadecimal>",
      "moves": [{ "id": 33, "level": 1 }],
      "evolutions": [{ "method": 4, "arg": 17, "species": "Dartrix" }] }
  ],
  "forms": [
    { "species": 3, "form": 2, "name": "Mega", "model_form": 0, "data": "<80 bytes, hexadecimal>" }
  ]
}
```

Rules:

- The **name is the key** of a species. It has 1 to 23 characters and is not a number. Two species with the same
  name cannot be installed together.
- A species reference is a name (a species of a pack) or a number from 1 to 721 (a species of the game).
- A form belongs to a species of the game or to a species of a pack, in this pack or in another one that is installed.
- `model_form` tells the plugin where the model is: 0 to 255 is a form of the first carrier species (Victini), 256 and
  more is a form of the second carrier (Volcanion), `0x8000 | n` is the model of the species `n` of the game.
- A pack has at most 24 level-up moves and 4 evolutions for each species.
- `credit` is shown by Studio before the installation.
- Studio checks the pack before it installs it: sizes, names, references.

## Registry on the SD card

Studio keeps `sdmc:/sango-studio/species/registry.json`. It lists, in the order of installation:

- the installed packs (id, name, version, active or removed),
- the species: the name and the pack. The **number is 724 plus the position in the list**,
- the forms: the species, the form number and the pack. The position is the order in which the plugin adds them.

A number never changes after installation, so a save file keeps working. A removed pack keeps its places: they
stay in the table as empty entries. Installing the pack again gives its numbers back. A pack that uses a species
of another pack needs that pack to be installed first.

## Files that the plugin reads

Studio writes these files at each installation. The plugin reads binary files only.

| File | Content |
|---|---|
| `species/table.bin` | the species (256 bytes each) and the forms (88 bytes each); the layout is in `module/pack/src/table.rs` |
| `species/packs/<id>.sangopack` | a copy of each installed pack (for Studio only) |
| `species/registry.json` | the numbers (for Studio only) |

`reload.bin` tells the plugin that the table changed. A new species needs a restart of the game, because the
game builds its species table at start.

## Plugin changes

1. `SpeciesTable` reads `species/table.bin` instead of the `.inc` files. The size of the table is
   fixed to a maximum (to decide, see below) and filled at run time.
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

## Migration of the existing data

1. A tool reads the `.inc` files and writes four packs: `gen7`, `gen8`, `regional-forms`,
   `z-a-megas`. The packs are the first content that users can install.
2. A test installs the four packs in an empty SD card and checks that the generated
   `table.bin` is byte-for-byte what the plugin builds today from the `.inc` files.
3. Only after this test, the `.inc` files leave the plugin.

## Open points

- **Maximum size of the table.** Today `kTotalEntries * 0x50` bytes are static. A table for 2 000
  entries is 160 KB. Check the free memory of the plugin (see the memory layout plan) before the
  choice.
- **Model files in packs.** Not done: a pack has no `.pc` file yet. Studio must copy the files, allocate slots and write `index.bin` and `0.raw`.
- **Model slots.** The carriers (Victini 494, Volcanion 721) have 255 slots each. Studio must allocate
  slots across all the packs. A third carrier may be needed.
- **Distribution.** Packs are files that users send to each other. A catalogue with download is
  possible later (a JSON index at an URL).
- **Third-party models.** Keep the credit and the licence of each model in the manifest. Studio
  shows them before the user installs a pack.
