# Layout

The **Layout** page shows the menus of the game (the Bag, the party, the Summary, the Box, the PSS, the battle menus...) and changes them.

## What the game has

The screens of the game are *layouts*. A layout is a list of **panes**: pictures, text boxes, windows and empty panes, in a tree. The layouts are in 111 archives of the romfs (399 layouts). A file of such an archive is a bundle: the layout, its images (`.bclim`) and its animations (`.bclan`), compressed with LZ11.

## The page

- **The list**: the layouts, grouped by menu. Type in the search box to find a layout or an archive (`a/0/6/1`).
- **The screen**: the layout at its real size (400 × 240 pixels for the top screen, 320 × 240 for the bottom one). Click a pane to select it. Drag it to move it. Drag a handle to resize it. The arrow keys move the pane by 1 pixel (with Shift, by 8). Delete removes it.
- **The tree and the properties** (right): the panes, with an eye for the hidden ones, and for the selected pane its position, its size, its scale, its rotation, its opacity, its origin and, for a text box, the size of the characters.
- **Duplicate**, **＋ Empty pane** and **Delete** add and remove panes. **Undo** and **Redo** go back and forth.
- **Replace the image with a PNG** changes the image of a picture. Every pane that uses the image changes.
- **Save for the game** writes `layout/<archive>/<file>.bin` on the SD card. **Back to the game layout** removes it.

## What the game does with a saved layout

The plugin reads `layout/<archive>/<file>.bin` in place of the file of the archive, like the fonts and the icons. **Restart the game** after a change. A file that has a version for each language cannot be saved (the plugin replaces a file by its number only).

## Limits

- The windows are drawn as rectangles, the texts show the text that the layout holds (the game writes its own text in many text boxes), and the animations are not played.
- The game finds a pane by its place in a table of names, not by its name. Moving, resizing and hiding a pane is safe. Deleting a pane that the game writes in (a name, a number) can stop the game: hide it instead. A new pane goes at the end of the table, so the game does not know it: it shows only what you put in it.
- Some images use formats that the page cannot read: they are empty on the screen (and keep their bytes in the file).
- Not tested in the game yet.
