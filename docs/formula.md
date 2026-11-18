# Formula and Nature

The **Formula** page changes how the game calculates the statistics of a Pokémon and the chance of a shiny Pokémon. The **Nature** page changes what each nature does, and adds new natures. Both pages write files on the SD card. The plugin reads them in the game, ORAS and XY. Nothing here is tested in the game yet.

## Statistics

The game calculates a statistic from five numbers: the base value of the species, the IV, the EV, the level, and (for the HP only) the species. The page shows the formula of each statistic, with the formula of the game as the first value:

| Statistic | Formula of the game |
| --- | --- |
| HP | `if(species == 292, 1, (2 * base + iv + ev / 4) * level / 100 + level + 10)` |
| Attack, Defense, Sp. Atk, Sp. Def, Speed | `(2 * base + iv + ev / 4) * level / 100 + 5` |

The formula gives the value **before the nature**. The nature then multiplies it (see below). To change a formula, write another text in the box. The page tells at once when the formula is wrong, and where. The formulas use whole numbers only: `7 / 2` is `3`.

| You can write | Meaning |
| --- | --- |
| `base`, `iv`, `ev`, `level`, `species` | The variables. |
| `+ - * / %` | The calculations (`%` is the remainder). A division by 0 gives 0. |
| `==`, `<`, `>` | Comparisons: they give 1 or 0. |
| `min(a, b)`, `max(a, b)` | The smaller and the bigger number. |
| `if(condition, then, else)` | `then` when the condition is not 0, else `else`. |

A formula has at most 120 bytes once it is compiled (the page shows the size) and at most 16 numbers at the same time. The result is limited to 1 to 65535.

## Shiny chance

The game makes a Pokémon shiny when a test that mixes the numbers of the Pokémon and of the trainer gives a number under 16, out of 65536 (1 out of 4096). The page lets you give the chance in percent: the limit becomes `percent × 65536 / 100`, so the step is about 0.0015 %. 100 % makes every Pokémon shiny. The setting **Shiny rate** of the menu of the plugin, when it is not off, has priority.

## Natures

The game raises one statistic by 10 % and lowers another one by 10 % (`value * 110 / 100`, `value * 90 / 100`). The table of the page has a **change in percent** for each nature and each of the five statistics (Attack, Defense, Sp. Atk, Sp. Def, Speed). You write the change itself: `10` raises by 10 %, `-10` lowers by 10 %, `50` raises by 50 %, `-200` lowers by 200 %, `0` changes nothing. The new value is `value * (100 + change) / 100`; when it is under 1 (for example with -200), the statistic is 1. A change is a whole number from -1000 to 1000.

**Add a nature** makes a new nature (the numbers 25 to 31). You give it a name in each language of the game (an empty name is the English name): the buttons "Names in" choose the language of the names that the table shows. You can rename the natures of the game in the same way; an empty name keeps the name of the game. The game shows the name in its own language where it writes the name of a nature (the plugin changes the text, ORAS only; this is not tested in the game yet). A new nature works for the statistics only: the game never gives it by chance, and the Trainer page does not offer it yet. The flavors and the arrows of the summary screen of the game know 25 natures only, and a few other screens read the names of the natures from another text of the game, so they do not show the name of a new nature.

## Files

| File | Content |
| --- | --- |
| `formula/formulas.txt` | The formulas that you changed and the shiny chance, as text (`attack = ...`, `shiny = 3.5`). The studio reads it. |
| `formula/formulas.bin` | The same formulas as programs, and the shiny limit. The plugin reads it (the layout is in `lib/include/pokemon/patch/studio_formulas.h` of the plugin). |
| `nature/natures.csv` | One line for each nature: `number,attack,defense,special_attack,special_defense,speed,name,ja,ja_kanji,fr,it,de,es,ko`. The five numbers are changes in percent; the eight names are the name in English and in the other languages (`25,0,-200,20,20,-30,Cosmic,,,Cosmique,,,,`). |

When everything is like the game, the files are removed. The studio tells the plugin to read the files again (`reload.bin`).

## How it works in the plugin

The plugin replaces the functions of the game that calculate the HP, Attack, Defense, Sp. Atk, Sp. Def and Speed, and the function that applies the nature. A statistic without a formula in the file keeps the code of the game. The formulas run in a small calculator of the plugin (`core::Formula`, tested on a computer with the same programs as the studio).
