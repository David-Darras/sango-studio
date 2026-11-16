# The cries

The cries of the Pokémon are in the archive 100 (`a/1/0/0`, 4594 files). A cry is two files.

| File | Content |
| --- | --- |
| Sequence (`CSEQ`, 3776 files, numbers 0 to 3775) | What to play: a note on the wave, with a label like `PV001`. It is the same for most species. |
| Wave archive (`CWAR`, 818 files, numbers from 3776) | The sound. Each one holds one wave (`CWAV`): mono, 4 bits for each sample (DSP-ADPCM), without loop, 32728 Hz for most. |

The numbers of the two files of a cry are in a table of the game code: for each species (and each form that has its own cry) the first row of its forms and four voices (Normal, Battle, Faint, Special). A voice has the number of the wave archive in the high 16 bits and the number of the sequence in the low 16 bits. The four kinds of a species usually use the same wave archive with other sequences. The table is in `module/cry/src/table.rs`.

## Studio

The cry card of the General tab (after the icon card) plays the cry, exports it as a WAV file, and replaces it by a WAV or MP3 file. An imported sound is made mono, brought to 32728 Hz, cut to 3 seconds at most (the files of the game last 2 seconds at most), encoded in 4 bits, and written as a wave archive in `species/cry/<file>.bin` on the SD card, with the list `species/cry/index.bin`. The plugin reads these files in place of the files of the game. Some cries share their wave archive: a change shows for all of them.

The encoder chooses 8 predictors for the sound (the groups of the predictors of its frames), then for each frame of 14 samples the predictor and the scale that leave the smallest error. A tone comes back with more than 28 dB between the signal and the noise (see the tests of `module/cry`). The files of the game are read and made again with the same layout: the same size and the same headers.

## Not done

- The species of the packs have no row in the table of the game, so they have no cry of their own yet.
- MP3 is read, not written: the export is a WAV file.
- Not tested in the game.
