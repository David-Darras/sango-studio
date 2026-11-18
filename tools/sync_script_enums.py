"""Gives each list of values of the script API an enum class in the Sango plugin.

  python tools/sync_script_enums.py <folder of the plugin repository>

It reads `tools/script_values.txt` (the lists, with a description of each value) and `tools/script_enum_map.txt` (which enum holds each list).
For an enum that the plugin already has, the names of the plugin are kept and the values that the plugin does not know are added to the
header. For the other lists, it writes a new header in `lib/include/<folder>/<name>.h`. It also writes `tools/script_value_names.txt`, the
name of the enumerator of each value, that `gen_script_natives.py` puts in the API.

The names are made from the description of the value: `Walk, 32 frames` gives `kWalk32Frames`. A header that already exists and is not
listed as `existing` is never overwritten.
"""

import os
import re
import subprocess
import sys
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))

LICENSE = """/*
 * Copyright (C) 2026  David Darras
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
"""


def read_lists():
    lists, order, current = {}, [], None
    for line in open(os.path.join(HERE, "script_values.txt"), encoding="utf-8"):
        line = line.rstrip("\r\n")
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
            lists[current] = []
            order.append(current)
        elif "|" in line and current:
            value, text = line.split("|", 1)
            lists[current].append((int(value), text))
    return lists, order


def read_map():
    rows, seen = [], set()
    for line in open(os.path.join(HERE, "script_enum_map.txt"), encoding="utf-8"):
        line = line.rstrip("\r\n")
        if not line or line.startswith("#"):
            continue
        parts = line.split("|")
        key = (parts[0], parts[1])
        if key in seen:
            continue
        seen.add(key)
        rows.append({"list": parts[0], "enum": parts[1], "folder": parts[2], "what": parts[3], "range": parts[4] if len(parts) > 4 else ""})
    return rows


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


PHRASES = [("smooth turn", "rotate"), ("a direction", ""), ("the direction", "facing"), ("grids", "tiles"), ("grid", "tile"), ("sp. atk", "special attack"),
           ("sp. def", "special defense"), ("the hands", "hands")]
STOP = {"a", "an", "the", "of", "to"}


def humanize(text, keep_notes=False):
    """The words of a description: `Walk, 32 frames (slow)` gives `Walk32Frames`."""
    if not keep_notes:
        text = re.sub(r"\([^)]*\)", " ", text)
    text = text.replace("(", " ").replace(")", " ")
    text = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode()
    low = text.lower()
    for old, new in PHRASES:
        low = low.replace(old, " " + new + " ")
    words = [w for w in re.split(r"[^a-z0-9]+", low) if w and w not in STOP]
    # Keep the capitals of the names (Mach Bike, Gogoat...): the words of the original text decide.
    original = {w.lower(): w for w in re.split(r"[^A-Za-z0-9]+", text) if w}
    words = [original.get(w, w) for w in words][:8]
    return "".join(w[0].upper() + w[1:] for w in words)


def read_overrides():
    found = {}
    path = os.path.join(HERE, "script_enumerator_names.txt")
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if line and not line.startswith("#"):
            name, value, enumerator = line.split("|")
            found[(name, int(value))] = enumerator
    return found


OVERRIDES = read_overrides()


def enumerators(items, taken=None, list_name=""):
    """The name of each value of a list, without two equal names. When two values have the same short name, all of them get the long name
    (with the words of the notes in parentheses)."""
    used = set(taken or [])
    short = {v: OVERRIDES.get((list_name, v)) or "k" + (humanize(t) or "Value") for v, t in items}
    counts = {}
    for name in short.values():
        counts[name] = counts.get(name, 0) + 1
    names = {}
    for value, text in items:
        name = short[value]
        if counts[name] > 1 or name in used:
            name = "k" + humanize(text, True)
        if name in used:
            name = "%s%d" % (name, value) if value >= 0 else name + "Negative"
        while name in used:
            name += "X"
        used.add(name)
        names[value] = name
    return names


def underlying(values):
    low, high = min(values), max(values)
    if low >= 0:
        return "u8" if high <= 0xFF else "u16" if high <= 0xFFFF else "u32"
    return "s8" if low >= -128 and high <= 127 else "s16" if low >= -32768 and high <= 32767 else "s32"


def in_range(value, text):
    if not text:
        return True
    low, high = [int(x) for x in text.split("-")]
    return low <= value <= high



def read_plugin_enum(path, enum):
    """The values of an enum of the plugin: (value, name, description). The description is the comment of the value, or its name in words."""
    text = open(path, encoding="utf-8").read()
    m = re.search(r"enum class %s\s*:\s*\w+\s*\{(.*?)\n\};" % enum, text, re.S)
    if not m:
        sys.exit("%s: enum %s not found" % (path, enum))
    items, next_value = [], 0
    for line in m.group(1).split("\n"):
        line = line.strip()
        em = re.match(r"(k\w+)\s*(?:=\s*(0x[0-9A-Fa-f]+|-?\d+))?\s*,?\s*(?:///<?\s*(.*))?$", line)
        if not em:
            continue
        value = int(em.group(2), 0) if em.group(2) else next_value
        next_value = value + 1
        if em.group(1) in ("kCount", "kFirstCustom", "kLastCustom"):
            continue
        words = re.findall(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+", em.group(1)[1:])
        description = (em.group(3) or "").strip().rstrip(".") or " ".join([words[0]] + [w.lower() for w in words[1:]])
        items.append((value, em.group(1), description))
    return items


def write_new(path, row, items, names, flags):
    namespace, enum = row["enum"].split("::")
    kind = underlying([v for v, _ in items])
    if flags and kind == "u8":
        kind = "u8"
    title = row["list"][0].lower() + row["list"][1:] if not row["list"][:2].isupper() else row["list"]
    lines = [LICENSE, "/**\n * @file %s.h\n * @brief The values that the scripts use for the %s.\n */\n\n#pragma once\n\n#include <types.h>\n\nnamespace %s {\n\n" % (snake(enum), title, namespace)]
    lines.append("/// The %s, as a script writes it.\nenum class %s : %s {\n" % (title, enum, kind))
    for value, text in items:
        shown = ("1 << %d" % (value.bit_length() - 1)) if flags else str(value)
        name = names[value]
        note = "" if humanize(text) == name[1:] else " ///< %s." % text.rstrip(".")
        lines.append("  %s = %s,%s\n" % (name, shown, note))
    lines.append("};\n\n} // namespace %s\n" % namespace)
    open(path, "w", encoding="utf-8", newline="\n").write("".join(lines))


def merge_existing(path, row, items):
    """Adds the values that the plugin does not know to an enum of the plugin; gives the name of each value."""
    text = open(path, encoding="utf-8").read()
    enum = row["enum"].split("::")[1]
    m = re.search(r"enum class %s\s*:\s*\w+\s*\{(.*?)\n\};" % enum, text, re.S)
    if not m:
        sys.exit("%s: enum %s not found" % (path, enum))
    body = m.group(1)
    known = {}
    for em in re.finditer(r"^\s*(k\w+)\s*=\s*(0x[0-9A-Fa-f]+|-?\d+)\s*,", body, re.M):
        if em.group(1) != "kCount":
            known[int(em.group(2), 0)] = em.group(1)
    taken = set(known.values()) | set(re.findall(r"^\s*(k\w+)", body, re.M))
    missing = [(v, t) for v, t in items if v not in known]
    added = enumerators(missing, taken, row["list"])
    if missing:
        block = "".join("  %s = %d, ///< %s.\n" % (added[v], v, t.rstrip(".")) for v, t in sorted(missing))
        count = re.search(r"^(\s*kCount\b.*\n)", body, re.M)
        new_body = body.replace(count.group(1), block + count.group(1)) if count else body.rstrip("\n") + "\n" + block.rstrip("\n")
        text = text[: m.start(1)] + new_body + text[m.end(1):]
        open(path, "w", encoding="utf-8", newline="\n").write(text)
    names = dict(known)
    names.update(added)
    return names, len(missing)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    plugin = sys.argv[1]
    lists, _ = read_lists()
    rows = read_map()
    plugin_lists = []
    out = ["# The enumerator of each value of the lists of the script API. Written by tools/sync_script_enums.py.\n"]
    for row in rows:
        items = [] if row["what"] == "plugin" else [(v, t) for v, t in lists[row["list"]] if in_range(v, row["range"])]
        namespace, enum = row["enum"].split("::")
        path = os.path.join(plugin, "lib", "include", *row["folder"].split("/"), snake(enum) + ".h")
        flags = len(items) > 1 and all(v > 0 and v & (v - 1) == 0 for v, _ in items)
        if row["what"] == "plugin":
            header = os.path.join(plugin, "lib", "include", *row["folder"].split("/"), row["range"] or snake(enum) + ".h")
            found = read_plugin_enum(header, enum)
            plugin_lists.append("[%s]\n" % row["list"] + "".join("%d|%s\n" % (v, d) for v, _, d in found))
            names = {v: n for v, n, _ in found}
            items = [(v, d) for v, _, d in found]
            print("%-28s plugin %s (%d values)" % (row["list"], row["enum"], len(found)))
        elif row["what"] == "existing":
            names, added = merge_existing(path, row, items)
            print("%-28s existing %s (%d added)" % (row["list"], row["enum"], added))
        else:
            # A header that git tracks is never rewritten: it belongs to the plugin (change `new` to `existing` in the map).
            tracked = os.path.exists(path) and subprocess.run(["git", "ls-files", "--error-unmatch", path], cwd=plugin, capture_output=True).returncode == 0
            if tracked:
                print("%-28s header is in git, kept: %s" % (row["list"], path))
                text = open(path, encoding="utf-8").read()
                names = {int(v, 0) if "x" in v else int(v): n for n, v in re.findall(r"^\s*(k\w+)\s*=\s*(0x[0-9A-Fa-f]+|-?\d+),", text, re.M)}
            else:
                names = enumerators(items, None, row["list"])
                os.makedirs(os.path.dirname(path), exist_ok=True)
                write_new(path, row, items, names, flags)
                print("%-28s new %s (%d values)" % (row["list"], row["enum"], len(items)))
        out.append("@%s|%s|%s\n" % (row["list"], row["enum"], row["range"]))
        for value, _ in items:
            if value in names:
                out.append("%s|%d|%s\n" % (row["list"], value, names[value]))
    open(os.path.join(HERE, "script_plugin_values.txt"), "w", encoding="utf-8", newline="\n").write("# The lists of values that are made from the enum classes of the plugin. Written by tools/sync_script_enums.py.\n" + "".join(plugin_lists))
    open(os.path.join(HERE, "script_value_names.txt"), "w", encoding="utf-8", newline="\n").write("".join(out))


main()
