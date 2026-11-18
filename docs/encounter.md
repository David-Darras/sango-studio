# The Encounter page

The Encounter page changes the wild Pokémon of each map, and the DexNav screen.

## The wild Pokémon

1. Choose a map in the menu. The maps with wild Pokémon come first (150 maps), and the maps that you changed have their own group.
2. Choose a kind of place: **Grass**, **Special grass**, **Extra**, **Surf**, **Rock Smash**, **Old Rod**, **Good Rod**, **Super Rod** or **Horde**. The tab glows when the map has wild Pokémon there.
3. Give a Pokémon, a form and two levels to each place. The page shows the chance that the game gives to each place:

| Kind | Places | Chance of each place |
| --- | --- | --- |
| Grass, Special grass | 12 | 10 %, 10 %, 10 %, 10 %, 10 %, 10 %, 10 %, 10 %, 10 %, 5 %, 4 %, 1 % |
| Extra | 3 | 60 %, 35 %, 5 % |
| Surf, Rock Smash | 5 | 50 %, 30 %, 15 %, 4 %, 1 % |
| Old Rod, Good Rod, Super Rod | 3 | 60 %, 35 %, 5 % |
| Horde | 3 hordes of 5 | 60 %, 35 %, 5 % for each horde |

The **rate** of a kind is the chance of a meeting at each step (0 turns the wild Pokémon off for this kind). For **Extra**, the choice tells if the game adds the three places to the ground or to the water. For **Rock Smash**, it tells if the rocks give the normal table or the fossils.

The **Summary** tab lists all the Pokémon that the player can meet on the map, with the total chance of each one.

The table goes in the file of the map on the SD card (`overworld/zone/<map>.bin`). The events that the Overworld page wrote in the same file stay. **Reset to the game** gives the game table back, and removes the file when nothing else changed in it. The DexNav shows the same table.

## The DexNav

The **DexNav** tab has two parts.

- **Screen of the map**: the bottom screen as the game draws it for the map that is open. The game gives each map a *type of screen* (21 types, one list in the archive 262): the type chooses the background picture and the places where the Pokémon go. The page draws the background of the type and puts the Pokémon of the table at its places: 15 places for the ground, 9 for the water and 6 for the fishing rods, in the order of the list. Drag a Pokémon (or an empty place), or type its numbers. All the maps with the same type use the new places. The page can show the other types too. The order of the Pokémon in the game's list is rebuilt from the table, so it can differ a little. These changes are not tested in the game.
- **Pictures**: the 33 pictures of the screen. **Export as PNG** writes one picture, and **Import a PNG** changes it. A picture that you import must have the same size and keeps the format of the game. **Export all the pictures** and **Import the pictures of a folder** do the same for all the pictures.

The places are in the 21 animations of the types of screen (the first value of each place), not in the layout: the layout has only the places before the animation.

**Save the DexNav** writes `dexnav/0.bin` on the SD card. The plugin reads it in place of the file of the game.
