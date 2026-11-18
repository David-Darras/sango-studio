# Trainers

The Trainer page changes all the trainers of the game: 949 trainers (the numbers 1 to 949) and 280 classes. It writes only what you change, on the SD card, and the plugin reads these files instead of the files of the game. Restart the game after a change.

## The files of the game

| Archive | File | Content |
| --- | --- | --- |
| 36 | one for each trainer | The data of the trainer (24 bytes): the data type of the team, the class, the kind of battle, the number of Pokémon, the 4 items, the AI (one bit for each way of thinking), the heal after the battle, the prize money and the gift item. |
| 38 | one for each trainer | The team: the Pokémon one after the other. A Pokémon is the IVs, the ability and the gender, the level, the species, the form, and then the held item (data type 2 and 3) and the four moves (data type 1 and 3): 8, 16, 10 or 18 bytes. |
| 37 | one for each class | The data of the class (16 bytes): the sequence of the 3D model, the gender, the group, the animation of the start of the battle, the 3D model, the 2D picture, the character on the map and the music of the sight. |
| 71 to 78, files 21 and 22 | one line for each class and for each trainer | The names of the classes and of the trainers, in the eight languages. |

All the files of the game read back and written again are the same bytes (a test checks the 949 trainers and the 280 classes).

## The files of the studio

`trainer/data/<n>.bin`, `trainer/pokemon/<n>.bin` and `trainer/class/<n>.bin`, with their lists `index.bin` (see `docs/storage.md`), and the texts in `game_text/<language>/21.bin` and `22.bin`. The module is `module/trainer`, the formats are in `module/core/src/trainer.rs`.

## The page

- **General**: the name in each language, the class, the kind of battle, the prize money, the heal after the battle, the gift item, the AI and the items of the battle.
- **Team**: 1 to 6 Pokémon. Each one has a species, a form, a level, IVs, an ability, a gender and, when the team chooses them, a held item and four moves. A team that does not choose its moves gives the Pokémon the moves of their level.
- **Class**: the data of the class. A change reaches all the trainers of this class.
- **CSV**: `trainers.csv` and `trainer_teams.csv` for all the trainers.
- **+ New trainer** takes a place of the game that has no name (a trainer that the game never uses) and starts it like the trainer that is open. The game has no room for more than 949 trainers.

## What the files of the game cannot hold: the extras

The files of the game give each Pokémon one value for the six IVs and nothing for the nature, the shiny state, the EVs and the nickname, and they cannot start a battle with a weather, a terrain or Trick Room. Studio keeps these in `trainer/extra/<n>.bin` (format `STRX`, see `module/core/src/trainer.rs`) and the plugin applies them when the battle with the trainer starts (`battle/patch/studio_trainers.h`):

- **Start of the battle** (tab General): the weather, the terrain (Grassy, Misty, Electric) and the effects on the whole field (Trick Room, Gravity, Magic Room, Wonder Room).
- **For each Pokémon** (tab Team, "Nature, EVs, IVs, shiny, nickname"): the nature, the shiny state, the exact ability, the six EVs, the six IVs and a nickname of up to 12 letters.

The effects of the field start when the first Pokémon enters the battle. The weather is the weather of the battle itself.

## The pictures

Each class has three 2D pictures in the archive 160: the whole body and two upper bodies (the files `3 * picture` to `3 * picture + 2`, where `picture` is the number of the 2D picture in the class). A picture is a BCLIM image in the format RGBA8, 512 x 256 or 256 x 256 pixels, compressed. The Class tab shows them, exports them as PNG and imports a PNG (it is resized to the size of the picture of the game). The files go to `trainer/picture/<file>.bin`.

## The messages

The messages of a trainer (before the battle, when it loses, after the battle...) are lines of the text file 23. The archive 135 tells which lines are the messages of a trainer: the file 1 is a list of 16-bit numbers (0 or the kind plus one) and the file 0 gives the place of each trainer in it; the line of a message is its place in the list. The Messages tab edits the texts of the messages that the game gives to the trainer, in the eight languages. A trainer cannot get a new kind of message: the list is in the game.

## Not done yet

- The 3D model of the class (the archive 133 holds the models of the battles).
- The effects on one side of the field (Reflect, Tailwind, the Spikes...) and the stat changes at the start.
- New kinds of messages.
- More than 949 trainers.

## The AI

The AI of a trainer chooses a move for each Pokémon like this: each move starts with 100 points, each way of thinking that is on gives its own points (a few are random: "+3 in 9 cases of 10"), and the trainer plays one of the moves that have the most points. The game has a script for six bits only:

| Bit | Name | What it does |
| --- | --- | --- |
| 0 | Basic | Removes the moves that would fail or do nothing (no effect, immune by ability, already asleep, stat at its limit...). |
| 1 | Attack | Prefers the move with the most damage, and the knock out or the x4 move. |
| 2 | Expert | The rules of a player for each kind of move (explosion at low HP, no more stat raises at +2, no waste against Protect...). |
| 7 | Double battle | The rules for the partner in the double battles. |
| 8 | Go easy | Spares the player: less points for damage when the Pokémon of the player has few HP. |
| 14 | Horde | The way of the Pokémon of a horde battle. |

The other bits have no script and change nothing. The page keeps them as they are in the file.

## The team builder

The Team tab works like the team builder of Pokémon Showdown, with the Gen 6 data of Smogon (`module/trainer/src/gen6.rs`, written by `node tools/gen_gen6.js <folder>` from the data files of Showdown and the Smogon sets).

- A slot for each Pokémon, and one editor: nickname, species (grouped by tier: Uber, OU, UU, RU, NU, PU, NFE, LC), level, gender, shiny, item, ability, nature, four moves, and the EVs and IVs with the stats at the level of the Pokémon.
- The menus put the useful things first: what the sets of the players use, then what the Pokémon can learn, then (Legal switch off) all the rest. The items start with the ones of the sets (strategic items).
- The Legal switch limits the menus to what is legal in Gen 6 and limits the EVs to 252 for a stat and 510 in all. A line under the editor lists what is not legal.
- "Copy the team" writes the team in the text format of Pokémon Showdown, "Import a team" reads it (see `showdown.rs`). A link opens the team builder of Showdown.

## The 3D model of a class

A class with a 3D model (Brendan, May, Steven...) has no 2D picture: the Class tab shows the model, which turns with the mouse. "Export the model (GLB)" writes the skeleton, the meshes and the textures. "Import a model (GLB)" replaces the model: the new file keeps the skeleton of the model of the game (the joints are found by name, so the motions of the game still move them), takes the materials of the model of the game, and puts its textures in the same file. The file `trainer/model/<5 x model>.bin` (compressed as the archive 133) is read by the plugin instead of the file of the game; "Back to the model of the game" removes it. The motions are not changed.
