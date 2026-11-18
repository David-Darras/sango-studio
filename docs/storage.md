# Where the files are

Sango Studio keeps all its files in a folder of the computer, the workspace: `%APPDATA%\SangoStudio\workspace` (the variable `SANGO_WORKSPACE` chooses another folder). The folder `sango-studio` of the SD card only gets the files that the plugin reads, and the studio copies them there each time that it writes them. The two folders have the same layout.

Each folder is named for what it holds. The files that replace a file of the game are named by the number of the file in the game archive (`98.bin`) and a folder also has `index.bin`, the list of its files for the plugin.

| Folder | Content | SD card |
| --- | --- | --- |
| `species/table.bin` | The species, the forms and the model places of the packs. | yes |
| `species/registry.json`, `species/packs/` | The numbers of the species of the packs, and the packs themselves (some hundreds of megabytes). | no |
| `species/data/<species>_<form>.bin` | The data of the species and of the forms that you changed. | yes |
| `species/pokedex_data.bin` | The Pokédex data (shapes and chains of forms). | yes |
| `species/icon/<file>.bin` | The icons of the species that you changed (archive 91). | yes |
| `species/cry/<file>.bin` | The wave archives of the cries that you replaced (archive 100). | yes |
| `species/level_up_moves/<file>.bin` | The level-up moves (archive 191): `<file>` is the place of the form in the archive. | yes |
| `species/egg_moves/<species>.bin` | The egg moves (archive 190). | yes |
| `species/evolution/<file>.bin` | The evolutions (archive 192): `<file>` is the place of the form in the archive. | yes |
| `trainer/data/<n>.bin`, `trainer/pokemon/<n>.bin`, `trainer/class/<n>.bin` | The data, the teams and the classes of the trainers that you changed (archives 36, 38 and 37). | yes |
| `leveling_rate/<rate>.bin` | The experience tables (archive 194). | yes |
| `game_text/<language>/<file>.bin` | The texts that you changed (archives 71 to 78): `ja`, `ja_kanji`, `en`, `fr`, `it`, `de`, `es`, `ko`. | yes |
| `ability/`, `move/data/`, `move/custom/`, `item/data/`, `item/custom/`, `type/` | The new and the changed abilities, moves, items and types. | yes |
| `species/model/` | The 3D models of the packs: the list `0.raw`, `index.bin` and the files `<n>.pc` (8 for each model). Made again from the packs. | yes, and only there |
| `code/`, `move/code/` | The code of the Code page. | no |
| `overworld/land`, `overworld/area`, `overworld/matrix`, `overworld/zone`, `overworld/model` | The blocks, the tilesets, the matrices, the zones (the events) and the models of the characters that the Overworld page writes. | yes, on the SD card only |
| `reload.bin` | The counter that tells the plugin that a file changed. | yes |

All that the studio writes is in the folder `sango-studio`, the 3D models too (they are only on the SD card, not on the computer). The older models in `sango/pokemodel` move in at the next opening.

The Overworld page keeps its own settings (the last block, the stars of the catalogue) in `overworld/` of the workspace. The scripts and the text files of the maps (`scripts.txt`, the events) stay in the folder `sango` of the SD card, because the editors of the plugin write them too.

When the studio opens the SD card, the maps that the older editor wrote in `sango` (`land`, `area`, `matrix`, `zone`, `overworld_model`) come to `overworld/`, and the files that older versions wrote there come to the workspace and get their new place (`pokemon/data` is now `species/data`, `pokemon/experience` is `leveling_rate`, `text` is `game_text`, the files by species and form of the moves and of the evolutions get the number of their file), and the SD card keeps only the files of the list above.
