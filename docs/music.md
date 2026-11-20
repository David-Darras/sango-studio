# The musics

The musics and the jingles of Alpha Sapphire are 199 stream files in the folder `sound` of the game (`.dspadpcm.bcstm`, 323 MB). The sound archive `sound/sango_sound.bcsar` (22 MB) lists 1709 sounds with their names. 256 of them are streams (211 musics and 45 jingles): each one has the name of a file of the folder. Some streams share a file.

## The files

| File | Content |
| --- | --- |
| `.bcstm` | A stream: 4 bits for each sample (DSP-ADPCM), one or two channels, a loop. See `module/music/src/bcstm.rs` for the layout. |
| `sango_sound.bcsar` | The names and the files of the sounds. See `module/music/src/bcsar.rs`. |

The number of a music (the number in parentheses in Studio) is the one that the scripts and the plugin use: 0 is the title theme, 1 is the evolution music. The numbers are in `module/music/src/ids.rs`, made by `tools/gen_music_ids.py` from a list of the sound ids of the game.

## Studio

The page Music lists the musics and the jingles as `name (number)`. For a music it plays the sound, exports it as a WAV file (the loop is in a `smpl` chunk), and replaces it with a WAV or an MP3 file:

- the sound is brought to 32728 Hz and to the number of channels of the stream of the game (a mono sound is copied, a stereo sound is mixed down),
- the loop starts where the user says (a WAV file with its own loop keeps it), and always ends at the end of the sound; the start is moved to the frame before it (14 samples),
- the file is made again with the layout of the game file and written as `sango/bgm/<name of the game file>` on the SD card.

The plugin plays a file of `sango/bgm` in place of the file of the game with the same name (see `core/patch/music_files.h`). The button "Put the music of the game back" removes the file.

## The sound effects

The tab "Sound effects" lists the 1436 sound effects of the archive as `Sound effect (number)`. A sound effect is a sequence that plays waves of a bank (`CBNK`). The file of a bank lists its waves (a wave archive and the place of the wave in it), and the wave archives (`CWAR`) are files of the sound archive. All the 1683 waves of the sound effects are mono waves of 4 bits. See `module/music/src/waves.rs`.

For a sound effect, the page lists its waves. It plays a wave, exports it as a WAV or an MP3 file, and replaces it:

- the new wave is made mono and written in the room of the old wave: the archive keeps its size and its layout, so a sound that is too long is cut (the page tells the longest length),
- the change is written in a copy of the archive, `sango/sound/sango_sound.bcsar` on the SD card (22 MB),
- "Put the wave of the game back" copies the old wave back; the copy is removed when nothing else is changed.

The number in parentheses is the one that the plugin gives to the sound functions: the first bank of the sound effects uses the number alone, the other sounds the whole id.

The plugin reads the copy: when it opens the sound archive to read a sound (`core/patch/music_files.h`), it reads the file of the SD card instead. The SD card is asked one time: restart the game after a change. The cries of the Pokémon are in the archive 100, not here (see `cries.md`).

## Export formats

WAV (16 bits, with the loop in a `smpl` chunk) and MP3 (the encoder LAME, under the LGPL licence: 192 kbit/s for two channels, 128 kbit/s for one channel, at 32000, 44100 or 48000 Hz). WAV and MP3 are read.

## Not done

- The musics made of sequences (a few musics of Pokémon-Amie) and the cry slots of the archive.
- The pages of the other games (X and Y) have other sound files.
- A wave cannot get a loop or more than one channel.
- Not tested in the game: the plugin hook that reads the copy of the archive is compiled only.
