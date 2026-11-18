"""Writes module/script/src/natives.rs: the native functions that the scripts of Omega Ruby and Alpha Sapphire can call.

The names and the parameters come from the tables of native functions of the field scripts of the game (the files `ScriptFuncSet*.cpp` of
the folder of the field scripts of the project) and from the functions of the language itself. The descriptions are in
`tools/script_native_notes.txt` (one line `Name|description`). The names that the scripts of the game call but that the tables of that
folder do not list (the functions of Pokémon-Amie and a few others) are in `tools/script_native_hashes.json`.

  python tools/gen_script_natives.py <folder of the field scripts>
"""

import glob
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(HERE), "module", "script", "src", "natives.rs")

# The names of the parameters that are written with Japanese words in the project: the names that the page shows.
RENAMES = {
    "keta": "digits", "waza": "move", "waza_no": "move", "waza_pos": "move_slot", "wazaNo": "move", "monsno": "species", "mons_no": "species", "formno": "form",
    "tokusei_id": "ability", "seikaku_id": "nature", "scrid": "script", "wkno": "work_number", "wkval": "work_value", "tbl_no": "table", "tbl_id": "table",
    "gx": "x", "gz": "z", "px": "x", "py": "y", "pz": "z", "goodsNo": "decoration", "goodsIndex": "decoration", "pointNo": "place", "secretBasePointNo": "place",
    "himitunoNakamaNo": "secret_friend", "bufId": "buffer", "buf_id": "buffer", "zone_id": "zone", "obj_id": "character", "objid": "character", "tar_obj_id": "target_character",
    "acmd": "action", "para_id": "parameter", "poke_index": "pokemon_slot", "sound_item_id": "sound", "eye_bgm": "music", "str_id": "text", "win_id": "window",
    "is_fade": "fade", "ret_wk": "result_work", "nuts_id": "berry", "item_no": "item", "item_id": "item", "pocket_no": "pocket", "tr_id": "trainer", "tr_no": "trainer",
    "flag_no": "flag", "work_no": "work", "idx": "index", "pos": "slot", "num": "count", "dir": "direction", "badge_id": "badge", "poroc_id": "poffin", "arc_id": "archive",
    "is_visible": "visible", "is_pause": "pause", "datid": "message", "type_id": "type", "rule_id": "rule", "ribbon_id": "ribbon", "bgm_no": "music", "demo_no": "cutscene",
}

# What the common parameters mean (the page shows this under the name).
MEANING = {
    "buffer": "The number of the text buffer that gets the value (the texts of the messages read these buffers).",
    "character": "The number of a character on the map (its number in the events of the zone).",
    "target_character": "The number of the character that is looked at or reached.",
    "script": "The number of a script.",
    "flag": "The number of an event flag.",
    "work": "The number of an event work value.",
    "zone": "The number of a zone (a map of the game).",
    "item": "The number of an item.",
    "species": "The number of a species (the number of the Pokédex).",
    "form": "The form of the species (0 is the normal form).",
    "move": "The number of a move.",
    "move_slot": "The place of the move in the four moves of the Pokémon (0 to 3).",
    "ability": "The number of an ability.",
    "nature": "The number of a nature.",
    "slot": "The place of the Pokémon in the team (0 is the first).",
    "pokemon_slot": "The place of the Pokémon in the team (0 is the first).",
    "x": "The position on the X axis (a cell of the grid, or a coordinate).",
    "y": "The height.",
    "z": "The position on the Z axis (a cell of the grid, or a coordinate).",
    "direction": "A direction (0 up, 1 down, 2 left, 3 right).",
    "count": "A number of things.",
    "digits": "How many digits the number is written with.",
    "text": "The number of a text in the message that is loaded.",
    "window": "The number of a message window.",
    "sound": "The number of a sound or of a music.",
    "music": "The number of a music.",
    "fade": "True to fade the screen in and out.",
    "visible": "True to show, false to hide.",
    "pause": "True to pause, false to go on.",
    "index": "A place in a list (0 is the first).",
    "badge": "The number of a badge (0 to 7).",
    "trainer": "The number of a trainer.",
    "berry": "The number of a Berry.",
    "pocket": "The number of a pocket of the Bag.",
    "result_work": "The event work value that receives the result.",
    "action": "The number of an action command.",
    "demo": "The number of a cutscene.",
    "cutscene": "The number of a cutscene.",
}

LANGUAGE = [
    ("float", [("value", "number", "The whole number to change.")], "Makes a decimal number from a whole number."),
    ("floatabs", [("value", "decimal", "The decimal number.")], "Gives the absolute value of a decimal number."),
    ("floatadd", [("first", "decimal", ""), ("second", "decimal", "")], "Adds two decimal numbers."),
    ("floatsub", [("first", "decimal", ""), ("second", "decimal", "The number that is taken from the first one.")], "Subtracts the second decimal number from the first."),
    ("floatmul", [("first", "decimal", ""), ("second", "decimal", "")], "Multiplies two decimal numbers."),
    ("floatdiv", [("dividend", "decimal", ""), ("divisor", "decimal", "Must not be 0.")], "Divides the first decimal number by the second."),
    ("floatcmp", [("first", "decimal", ""), ("second", "decimal", "")], "Compares two decimal numbers: gives -1, 0 or 1."),
    ("floatround", [("value", "decimal", ""), ("mode", "number", "0 rounds to the nearest, 1 down, 2 up, 3 toward zero.")], "Rounds a decimal number to a whole number."),
    ("floatsin", [("value", "decimal", "The angle."), ("mode", "number", "0 radians, 1 degrees, 2 grades.")], "Gives the sine of an angle."),
    ("floatcos", [("value", "decimal", "The angle."), ("mode", "number", "0 radians, 1 degrees, 2 grades.")], "Gives the cosine of an angle."),
]


def read(path):
    return open(path, "rb").read().decode("cp932", errors="replace")


KINDS = {
    "u8": "number from 0 to 255",
    "u16": "number from 0 to 65535",
    "s16": "number from -32768 to 32767",
    "u32": "whole number",
    "s32": "whole number",
    "int": "whole number",
    "cell": "whole number",
    "float": "decimal",
    "f32": "decimal",
    "bool": "yes or no",
    "b32": "yes or no",
    "b8": "yes or no",
    "BOOL": "yes or no",
}
# The kinds and the lists of the types of the game. These names come from the source of the game, so they stay in a file outside the repository
# (the second argument of the script, by default `private/tools/script_types.json` of the plugin repository).
PRIVATE = {"kinds": {}, "lists": {}}


def kind_of(ctype):
    ctype = (ctype or "").strip()
    if ctype in KINDS:
        return KINDS[ctype]
    if ctype in PRIVATE["kinds"]:
        return PRIVATE["kinds"][ctype]
    if not ctype:
        return "whole number"
    return "value from a list of the game"


def main():
    source = sys.argv[1] if len(sys.argv) > 1 else sys.exit(__doc__)
    types_path = sys.argv[2] if len(sys.argv) > 2 else r"C:\Users\David\CLionProjects\ctrpluginframework\sango_plugin\private\tools\script_types.json"
    if os.path.exists(types_path):
        PRIVATE.update(json.load(open(types_path, encoding="utf-8")))
    texts = {n: read(os.path.join(source, n)) for n in sorted(os.listdir(source)) if n.endswith((".cpp", ".h"))}
    tables = {}
    for name, text in texts.items():
        if name.startswith("ScriptFuncSet") and name.endswith(".cpp"):
            for m in re.finditer(r"^[ 	]*GFL_PAWN_FUNC(?:_[A-Z_]*)?\(\s*(\w+)\s*\)", text, re.M):
                tables.setdefault(name[len("ScriptFuncSet"):-4], []).append(m.group(1))
    functions = {}
    pattern = re.compile(r"cell\s+(?:AMX_NATIVE_CALL\s+)?(?:\w+::)?(\w+)\s*\(\s*AMX\s*\*\s*\w*\s*,\s*const\s+cell\s*\*\s*(\w*)\s*\)\s*\{")
    for text in texts.values():
        for m in pattern.finditer(text):
            name, ptr = m.group(1), m.group(2)
            depth, i = 1, m.end()
            while i < len(text) and depth:
                depth += {"{": 1, "}": -1}.get(text[i], 0)
                i += 1
            body = text[m.end():i]
            found = {}
            if not ptr:
                functions[name] = []
                continue
            for pm in re.finditer(r"(?:(\w+(?:::\w+)?)\s+)?(\w+)\s*=\s*(?:\(\s*([\w:\s\*]+?)\s*\)\s*|static_cast\s*<\s*([\w:\s\*]+?)\s*>\s*\(\s*)?(amx_ctof\s*\(\s*)?" + ptr + r"\s*\[\s*(\d+)\s*\]", body):
                ctype, pname, cast, scast, flt, index = pm.group(1), pm.group(2), pm.group(3), pm.group(4), pm.group(5), int(pm.group(6))
                found.setdefault(index, (pname, "float" if flt else (cast or scast or ctype or "")))
            used = sorted({int(n) for n in re.findall(ptr + r"\s*\[\s*(\d+)\s*\]", body)})
            count = max(used) if used else 0
            functions[name] = [found.get(k, ("", "")) for k in range(1, count + 1)]
    notes = {}
    for line in open(os.path.join(HERE, "script_native_notes.txt"), encoding="utf-8"):
        line = line.rstrip("\r\n")
        if "|" in line:
            key, text = line.split("|", 1)
            notes[key] = text
    names_help = {}
    for line in open(os.path.join(HERE, "script_param_names.txt"), encoding="utf-8"):
        if "|" in line:
            key, text = line.rstrip("\r\n").split("|", 1)
            names_help[key] = text
    docs, returns = {}, {}
    for line in open(os.path.join(HERE, "script_native_docs.txt"), encoding="utf-8"):
        line = line.rstrip("\r\n")
        if "|" in line and "." in line.split("|", 1)[0]:
            key, text = line.split("|", 1)
            fn, what = key.split(".", 1)
            if what == "@return":
                returns[fn] = (returns.get(fn, "") + " " + text).strip()
            else:
                docs[(fn, what)] = text
    lists, order = {}, []
    current = None
    for line in open(os.path.join(HERE, "script_values.txt"), encoding="utf-8"):
        line = line.rstrip("\r\n")
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
            lists[current] = []
            order.append(current)
        elif "|" in line and current:
            value, text = line.split("|", 1)
            lists[current].append((int(value), text))
    links = {}
    for line in open(os.path.join(HERE, "script_value_params.txt"), encoding="utf-8"):
        if "|" in line:
            key, text = line.rstrip("\r\n").split("|", 1)
            fn, what = key.split(".", 1)
            links[(fn, what)] = text
            assert text in lists, text
    # The natives that are not in the tables: the Pokémon-Amie script and the library. Their parameters come from the declarations (`native Name(...)`)
    # in the include files of their scripts.
    base = os.path.abspath(os.path.join(source, "..", "..", "..", ".."))
    declared = {}
    for relative, group in (("prog/src/*/Pawn/Native.inc", "Pokémon-Amie"), ("prog/include/pawn/include/PawnObject.inc", "Pokémon-Amie"), ("lib/opensource/pawn/include/include/gfl_func.inc", "Language"), ("prog/src/*/*/*_ai_common.inc", "Trainer battle AI")):
        found_paths = glob.glob(os.path.join(base, *relative.split("/")))
        if not found_paths:
            continue
        path = found_paths[0]
        for line in read(path).split("\n"):
            dm = re.match(r"\s*native\s+(?:\w+:)?(\w+)\s*\(([^)]*)\)", line)
            if dm and dm.group(1) not in declared:
                items = []
                for raw in [x.strip() for x in dm.group(2).split(",") if x.strip()]:
                    decimal = raw.startswith("Float:")
                    raw = raw.replace("Float:", "").strip()
                    pname = raw.split("=")[0].strip()
                    flag = pname in ("flag", "on_off", "small_big", "force", "replace_quit")
                    items.append((pname, "decimal" if decimal else ("yes or no" if flag else "whole number"), None))
                declared[dm.group(1)] = (group, items)
    others = json.load(open(os.path.join(HERE, "script_native_hashes.json"), encoding="utf-8"))
    listed = {n for names in tables.values() for n in names}
    extra = sorted({n for n in others.values() if n not in listed and n not in {l[0] for l in LANGUAGE}})
    out = ["//! The native functions that the scripts of the game can call.\n//!\n//! Generated by `tools/gen_script_natives.py`. Do not edit by hand.\n\n"]
    out.append("/// A parameter of a native function: its name, the kind of value that it takes and what it means.\n#[derive(Clone, Copy, Debug)]\npub struct Param {\n    pub name: &'static str,\n    pub kind: &'static str,\n    pub help: &'static str,\n    /// The name of the list of values that the parameter takes, or an empty text.\n    pub list: &'static str,\n}\n\n/// A list of values with their meaning (a direction, a kind of exit...).\n#[derive(Clone, Copy, Debug)]\npub struct ValueList {\n    pub name: &'static str,\n    pub items: &'static [(i64, &'static str)],\n}\n\n")
    out.append("/// A native function: the name that the game uses, the group, the parameters and what it does. `known` is false when the parameters are not\n/// documented (the function is not described in the tables of the field scripts).\n#[derive(Clone, Copy, Debug)]\npub struct Native {\n    pub name: &'static str,\n    pub group: &'static str,\n    pub params: &'static [Param],\n    pub known: bool,\n    pub help: &'static str,\n    pub returns: &'static str,\n}\n\n")

    def q(text):
        return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'

    def params_of(items, fn=""):
        rows = []
        for k, item in enumerate(items):
            name, kind, help_text = item if len(item) == 3 else (item[0], kind_of(item[1]), None)
            name = RENAMES.get(name, name) if name else "value%d" % (k + 1)
            original = item[0]
            text = docs.get((fn, original)) or (help_text if help_text is not None else MEANING.get(name, "")) or names_help.get(original, "") or names_help.get(name, "")
            ctype = item[1] if len(item) == 2 else ""
            listed = links.get((fn, original)) or PRIVATE["lists"].get(ctype, "")
            rows.append('Param { name: %s, kind: %s, help: %s, list: %s }' % (q(name), q(kind), q(text), q(listed)))
        return ", ".join(rows)

    out.append("pub const NATIVES: &[Native] = &[\n")
    count = described = known = 0
    for group in sorted(tables):
        for name in tables[group]:
            items = functions.get(name)
            help_text = notes.get(name, "")
            described += bool(help_text)
            known += items is not None
            out.append("    Native { name: %s, group: %s, params: &[%s], known: %s, help: %s, returns: %s },\n" % (q(name), q(group), params_of(items or [], name), "true" if items is not None else "false", q(help_text), q(returns.get(name, ""))))
            count += 1
    for name, params, help_text in LANGUAGE:
        out.append("    Native { name: %s, group: \"Language\", params: &[%s], known: true, help: %s, returns: \"\" },\n" % (q(name), params_of(params), q(help_text)))
        count += 1
        described += 1
        known += 1
    for name in extra:
        group, items = declared.get(name, ("Pokémon-Amie", None))
        out.append("    Native { name: %s, group: %s, params: &[%s], known: %s, help: %s, returns: %s },\n" % (q(name), q(group), params_of(items or [], name), "true" if items is not None else "false", q(notes.get(name, "")), q(returns.get(name, ""))))
        described += bool(notes.get(name))
        known += items is not None
        count += 1
    out.append("];\n\n")
    out.append("/// The lists of values.\npub const LISTS: &[ValueList] = &[\n")
    for name in order:
        out.append("    ValueList { name: %s, items: &[%s] },\n" % (q(name), ", ".join("(%d, %s)" % (v, q(t)) for v, t in lists[name])))
    out.append("];\n\n")
    out.append("/// The list with this name.\npub fn list(name: &str) -> Option<&'static ValueList> {\n    LISTS.iter().find(|l| l.name == name)\n}\n\n")
    out.append("/// The number that the game writes in a script for the name of a native function (and of a public function).\npub fn hash(name: &str) -> u32 {\n    name.bytes().fold(0u32, |value, byte| value.wrapping_mul(0x83) ^ u32::from(byte))\n}\n")
    open(OUT, "w", encoding="utf-8", newline="\n").write("".join(out))
    print(len(returns), "with a return text;", len(docs), "parameter texts;", count, "natives;", described, "described;", known, "with parameters")


main()
