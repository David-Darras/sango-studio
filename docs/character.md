# Character and Swap

The **Character** page shows the 3D models of the characters of the overworld (May, Brendan, the Gym Leaders, the people of the maps), exports them and imports other ones. Its **Swap** tab makes a character look like another one. Nothing here is tested in the game yet.

## The models

The game keeps the models of the characters in the archive 21: one pack for each character. A pack has the 3D model with its pictures, usually the motions of the character (the walk, the run, the jumps, the waves... between 4 and 80 motions, from a list of 238 places), and a small model. The names in the lists come from `overworld::ModelId` of the plugin.

## Export

**Export the model (GLB)** writes a GLB file with:

- the skeleton (one node for each joint),
- the meshes with their weights and their pictures,
- one animation for each motion of the character, named `motion 12`, `motion 13`... (30 frames each second). The motions are the real walk, run, jump and other motions of the game. The names of the motions are not known yet, so the number of the place is the name.

## Import

**Import a model (GLB)…** replaces the model of the chosen character with the model of a glTF or GLB file. The skeleton and the motions of the game stay: the motions move the joints by their names. So export the model first, change it in Blender (the meshes, the pictures), keep the names of the joints, and import it. The scale box changes the size of the model.

The animations of the file are not read: the character keeps the motions of the game.

## Swap

**Swap** makes a character look like another one: the pack of the first character becomes a copy of the pack of the second one (the model and its motions). Every place where the game shows the first character shows the second one. **Swap both** trades the two models. The list at the bottom shows the characters that are changed, with a button to give each model back.

The player is made of the models "May" and "Brendan" and of other models that the game dresses. Swap changes the models that are listed; whether the player of a game in progress uses these models is not checked.

## Files

The changes are whole packs on the SD card: `overworld/model/<pack>.bin` and the list `overworld/model/index.bin`. The plugin gives the file to the game in place of the pack. Restart the game after a change. The packs of the game are never changed.
