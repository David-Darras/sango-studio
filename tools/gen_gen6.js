// Writes module/trainer/src/gen6.rs and module/trainer/src/gen6_learn.bin: the Gen 6 data of Smogon.
//
// The input is a folder with the data files of Pokemon Showdown (learnsets.js, pokedex.js, items.js,
// moves.js, abilities.js), the file data/mods/gen6/formats-data.ts of the Smogon repository
// (as gen6_formats.ts) and the sets of Smogon (sets_gen6.json).
//
//   node tools/gen_gen6.js <folder>
//
// What it writes:
//   - the tier of each species (Uber, OU, UU...),
//   - the moves that each species can learn in Gen 6 (a bit for each move, with the earlier forms),
//   - the sets of the players (item, ability, nature, moves, EVs, IVs) with the ids of the game,
//   - the items of the sets, the most used first.
// The ids of the game are the numbers of Showdown: the tool checks the names against the lists of
// the Species page and stops at the first difference.

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const folder = process.argv[2];
if (!folder) {
  console.error('usage: node tools/gen_gen6.js <folder>');
  process.exit(1);
}
const root = path.resolve(__dirname, '..');
const outRs = path.join(root, 'module', 'trainer', 'src', 'gen6.rs');
const outBin = path.join(root, 'module', 'trainer', 'src', 'gen6_learn.bin');

function load(file, name) {
  const box = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(folder, file), 'utf8'), box);
  return box.exports[name];
}
const dex = load('pokedex.js', 'BattlePokedex');
const learnsets = load('learnsets.js', 'BattleLearnsets');
const items = load('items.js', 'BattleItems');
const moves = load('moves.js', 'BattleMovedex');
const abilities = load('abilities.js', 'BattleAbilities');
const sets = JSON.parse(fs.readFileSync(path.join(folder, 'sets_gen6.json'), 'utf8'));
const formats = fs.readFileSync(path.join(folder, 'gen6_formats.ts'), 'utf8');

// The names of the game.
function names(file, constant) {
  const text = fs.readFileSync(path.join(root, 'module', 'species', 'src', file), 'utf8');
  const start = text.indexOf('pub const ' + constant);
  const body = text.slice(text.indexOf('[', text.indexOf('=', start)), text.indexOf('];', start));
  return [...body.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => m[1].replace(/\\"/g, '"'));
}
const speciesNames = names('names.rs', 'SPECIES');
const itemNames = names('names.rs', 'ITEM');
const abilityNames = names('names.rs', 'ABILITY');
const moveNames = names('move_names.rs', 'MOVE');
let mismatches = 0;
const id = (s) => String(s).toLowerCase().replace(/[^a-z0-9]/g, '');

const tierNames = ['Uber', 'OU', 'UUBL', 'UU', 'RUBL', 'RU', 'NUBL', 'NU', 'PUBL', 'PU', 'ZUBL', 'ZU', 'NFE', 'LC', 'Other'];
const tierOf = (t) => {
  t = t.replace(/[()]/g, '');
  if (t === 'AG') return 0;
  const at = tierNames.indexOf(t);
  return at < 0 ? tierNames.length - 1 : at;
};

// The tier of each species: the entry of the base species.
const tiers = new Array(speciesNames.length + 1).fill(tierNames.length - 1);
const formatEntries = {};
for (const m of formats.matchAll(/\n\t(\w+): \{\s*tier: "([^"]+)"/g)) formatEntries[m[1]] = m[2];
const byNum = {};
for (const [key, e] of Object.entries(dex)) if (e.num > 0 && !e.forme && e.num <= speciesNames.length) byNum[e.num] = key;
for (let n = 1; n <= speciesNames.length; n++) {
  const key = byNum[n];
  if (key && formatEntries[key]) tiers[n] = tierOf(formatEntries[key]);
}

// The moves of Gen 6.
const moveById = {};
for (const [key, m] of Object.entries(moves)) {
  if (m.num > 0 && m.num <= moveNames.length && !m.isZ && !m.isMax) moveById[key] = m.num;
}
for (const [key, n] of Object.entries(moveById)) {
  if (!key.startsWith('hiddenpower') && id(moveNames[n - 1]) !== key && id(moveNames[n - 1]) !== id(moves[key].name)) {
    console.error('move difference', n, key, moveNames[n - 1]);
    mismatches++;
  }
}
const legalIn6 = (code) => {
  const gen = parseInt(code[0], 10);
  return gen >= 1 && gen <= 6;
};
const bytesPerSpecies = Math.ceil(moveNames.length / 8);
const learn = Buffer.alloc((speciesNames.length + 1) * bytesPerSpecies);
function ownMoves(key, found) {
  const entry = learnsets[key];
  if (!entry || !entry.learnset) return;
  for (const [move, codes] of Object.entries(entry.learnset)) {
    if (moveById[move] && codes.some(legalIn6)) found.add(moveById[move]);
  }
}
for (let n = 1; n <= speciesNames.length; n++) {
  let key = byNum[n];
  const found = new Set();
  const seen = new Set();
  while (key && !seen.has(key)) {
    seen.add(key);
    ownMoves(key, found);
    const prevo = dex[key].prevo;
    key = prevo ? id(prevo) : null;
  }
  for (const move of found) {
    const bit = move - 1;
    learn[n * bytesPerSpecies + (bit >> 3)] |= 1 << (bit & 7);
  }
}
fs.writeFileSync(outBin, learn);

// The items and the abilities by name.
const itemByName = {};
for (const it of Object.values(items)) {
  if (it.num > 0 && it.num <= itemNames.length) {
    if (id(itemNames[it.num - 1]) !== id(it.name)) {
      console.error('item difference', it.num, it.name, itemNames[it.num - 1]);
    mismatches++;
    }
    itemByName[id(it.name)] = it.num;
  }
}
const abilityByName = {};
for (const a of Object.values(abilities)) {
  if (a.num > 0 && a.num <= abilityNames.length) {
    if (id(abilityNames[a.num - 1]) !== id(a.name)) {
      console.error('ability difference', a.num, a.name, abilityNames[a.num - 1]);
    mismatches++;
    }
    abilityByName[id(a.name)] = a.num;
  }
}

const natures = ['Hardy', 'Lonely', 'Brave', 'Adamant', 'Naughty', 'Bold', 'Docile', 'Relaxed', 'Impish', 'Lax', 'Timid', 'Hasty', 'Serious', 'Jolly', 'Naive', 'Modest', 'Mild', 'Quiet', 'Bashful', 'Rash', 'Calm', 'Gentle', 'Sassy', 'Careful', 'Quirky'];
const first = (v) => (Array.isArray(v) ? v[0] : v);
const stat = (o, k, d) => (o && o[k] !== undefined ? first(o[k]) : d);
const order = ['hp', 'atk', 'def', 'spa', 'spd', 'spe'];

// The sets, once for each species and name.
const setsBySpecies = {};
const itemUse = {};
for (const group of Object.values(sets)) {
  for (const [speciesName, byName] of Object.entries(group.dex || {})) {
    const key = id(speciesName);
    const n = dex[key] && dex[key].num;
    if (!(n > 0) || n > speciesNames.length || dex[key].forme) continue;
    for (const [setName, s] of Object.entries(byName)) {
      const item = itemByName[id(first(s.item) || '')] || 0;
      const ability = abilityByName[id(first(s.ability) || '')] || 0;
      const nature = Math.max(0, natures.indexOf(first(s.nature) || 'Hardy'));
      const list = (s.moves || []).map((m) => moveById[id(first(m).replace(/ \d+$/, ''))] || 0).filter((m) => m);
      const unique = [...new Set(list)].slice(0, 4);
      if (!unique.length) continue;
      const evs = order.map((k) => stat(s.evs, k, 0));
      const ivs = order.map((k) => stat(s.ivs, k, 31));
      const known = (setsBySpecies[n] = setsBySpecies[n] || []);
      const name = setName.replace(/"/g, '').slice(0, 40);
      if (known.some((o) => o.name === name && o.item === item && o.unique.join() === unique.join())) continue;
      known.push({ name, item, ability, nature, unique, evs, ivs });
      if (item) itemUse[item] = (itemUse[item] || 0) + 1;
    }
  }
}
const strategic = Object.entries(itemUse).sort((a, b) => b[1] - a[1]).map(([item]) => Number(item));

let rs = '//! The Gen 6 data of Smogon: tiers, the moves that each species learns, and the sets of the players.\n//!\n';
rs += '//! Generated by `tools/gen_gen6.js`. Do not edit by hand.\n\n';
rs += `pub const TIER_NAMES: [&str; ${tierNames.length}] = [${tierNames.map((t) => JSON.stringify(t)).join(', ')}];\n\n`;
rs += '/// The tier of each species, from the species 1 on (the index is the number of the species; 0 is not used).\n';
rs += `pub static TIERS: [u8; ${tiers.length}] = [${tiers.join(', ')}];\n\n`;
rs += `/// The bytes of the learn table for each species: one bit for each move (the move 1 is the bit 0).\npub const LEARN_BYTES: usize = ${bytesPerSpecies};\n`;
rs += '/// The moves that each species learns in Gen 6, with the earlier forms of the species.\npub static LEARN: &[u8] = include_bytes!("gen6_learn.bin");\n\n';
rs += '/// A set of the players: a name, the item, the ability (0 is none), the nature, four moves (0 is none), the EVs and the IVs.\n';
rs += '#[derive(Debug, Clone, Copy)]\npub struct Set {\n    pub name: &\'static str,\n    pub item: u16,\n    pub ability: u16,\n    pub nature: u8,\n    pub moves: [u16; 4],\n    pub evs: [u8; 6],\n    pub ivs: [u8; 6],\n}\n\n';
rs += '/// The items of the sets, the most used first.\n';
rs += `pub static STRATEGIC_ITEMS: [u16; ${strategic.length}] = [${strategic.join(', ')}];\n\n`;
const speciesWithSets = Object.keys(setsBySpecies).map(Number).sort((a, b) => a - b);
rs += '/// The sets of each species that has some, by number of species.\n';
rs += `pub static SETS: [(u16, &[Set]); ${speciesWithSets.length}] = [\n`;
for (const n of speciesWithSets) {
  rs += `    (${n}, &[\n`;
  for (const s of setsBySpecies[n]) {
    const mv = [0, 1, 2, 3].map((i) => s.unique[i] || 0);
    rs += `        Set { name: ${JSON.stringify(s.name)}, item: ${s.item}, ability: ${s.ability}, nature: ${s.nature}, moves: [${mv.join(', ')}], evs: [${s.evs.join(', ')}], ivs: [${s.ivs.join(', ')}] },\n`;
  }
  rs += '    ]),\n';
}
rs += '];\n';
fs.writeFileSync(outRs, rs);
console.log('species with sets', speciesWithSets.length, 'items', strategic.length, 'learn bytes', learn.length);
console.log('name differences', mismatches);
