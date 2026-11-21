# Font

The **Font** page shows the fonts of the game and replaces them. Replacing the battle font (2) is **tested in the game and works**. The other fonts are not tested.

## What the game has

The fonts are in the archive 167 (`a/1/6/7`). The files are font files of the 3DS (`CFNT`, the `.bcfnt` format), not compressed. The roles come from the index of the game files and from David, and were checked on the glyphs in the dump.

| File | Cell | Format | Glyphs | Role |
| --- | --- | --- | --- | --- |
| 0 | 24×30 | A4 | 3315 | System font for the Japanese, US and European versions (the index of the game files names it so). |
| 1 | 24×30 | A4 | 2865 | System font for the Korean version. |
| 2 | 13×15 | LA4 | 2880 | Battle font (checked in the game: a replaced font 2 works): white letters with a black outline (Latin, kana, symbols). It is the font of the nicknames in battle (David; the loading code of the game creates it when a battle starts). |
| 3 | 13×11 (11, 10 or 15 wide for some languages) | LA4 | 32 | Number font, one file for each language (files 0, 2, 3, 4, 5, 7, 8): the digits, `Lv`, `?`, `!` and a few symbols, for HP and levels. The names of the languages follow the order of the 3DS language codes: this is a guess. |
| 4 | 37×23 | LA4 | 240 | Braille: the glyphs are the dot patterns of 6 and 8 dots (the index of the game files calls it by the Japanese word for braille). |

## The page

- A button for each font, with its role. The page draws the glyphs, and a text that you write, with the widths of the font.
- Click a glyph to see its character code and its widths.
- **Export the sheet** saves a PNG picture of all the glyphs and a `.csv` file with the widths (see below). Draw over the PNG with any picture editor.
- **Import a sheet** reads the PNG (and the `.csv` with the same name) and shows the new font. Nothing is written until you click **Save for the game**.
- **Make the glyphs from a font of the computer** lists the `.ttf` and `.otf` files of `C:\Windows\Fonts` and of the fonts of the user (or any file that you choose). You choose the size in pixels, whether to draw the Latin characters only, and (for the fonts with white letters) the black outline. The program draws every character that the game font has, on the baseline of the cell. The symbols, and the characters that the computer font does not have, keep the picture of the game. Then you can export the sheet and retouch it.
- **Export .bcfnt** and **Use a .bcfnt file** work with the font file itself (for example one made with the font converter of the 3DS tools).
- **Save for the game** writes `font/<n>.bin` on the SD card. The plugin reads this file in place of the font of the game. Restart the game after a change. **Back to the game font** removes it.
- The number fonts (file 3, one file for each language) can be drawn on and exported, but not saved for the game: the plugin replaces a file by its number only.

## The sheet

The sheet looks like the pictures of the font converter: blocks of `cell width + 4` by `cell height + 5` pixels, 16 on a line, one for each glyph in the order of the font. A block has a black border, a grey-green margin, the cell (the picture of the glyph), and under it a red bar that starts at the space on the left of the glyph and is as long as the width of the character. A blue block has no glyph.

- Fonts A4: the cell is white and the ink is dark; a darker pixel is more opaque (16 levels).
- Fonts LA4: the cell has the real colors and alpha (16 levels each). The pixels that you do not draw are transparent.
- The `.csv` file has one line for each glyph: `index,code,left,glyph,advance` (the space on the left, the width of the picture, the width of the character). The import reads the widths from it. Without the `.csv` file, the import reads the red bars (space on the left, width of the character) and the right edge of the ink (width of the picture).
- The size of the sheet and of the cells must stay the same: the font keeps its layout, its map of characters and its size in bytes. Only the pictures and the widths change. The page cannot add glyphs or change the size of the cells.

Tested on the dump: the sheet of each font gives, read again, the same file byte for byte; a drawn pixel and a changed width are written; a computer font (Arial) makes valid fonts.

## Limits

- The system fonts (0 and 1) are loaded when the game starts. The plugin may start after that, so a replaced system font may need a way to load it again. The battle font (2) and the number font (3) are loaded when a battle starts, so test with them first. Check `sdmc:/sango-studio/overworld/model/trace.log`: a line `1 2 <size>` means that the game asked for the size of the replaced font 2.

- Only the battle font (2) is tested in the game. The others are not.
- It is not known which screen uses which font. Try the system font first and look.
- The generator draws plain letters: no hinting for small sizes, so a hand retouch is often needed for a font of 11 to 15 pixels.
