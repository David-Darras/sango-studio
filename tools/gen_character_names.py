"""Writes module/character/src/names.rs: the names of the models of the characters of the overworld.

The names come from the enum class `overworld::ModelId` of the Sango plugin (lib/include/overworld/constant/model.h).

  python tools/gen_character_names.py <folder of the plugin repository>
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(HERE), "module", "character", "src", "names.rs")


def words(name):
    parts = re.findall(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+", name)
    return " ".join([parts[0]] + [p if p.isupper() and len(p) > 1 else p.lower() if not p[0].isupper() or True else p for p in parts[1:]]) if parts else name


def pretty(name):
    parts = re.findall(r"[A-Z]+(?![a-z])|[A-Z][a-z]+|[a-z]+|\d+", name)
    return " ".join(parts)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    path = os.path.join(sys.argv[1], "lib", "include", "overworld", "constant", "model.h")
    text = open(path, encoding="utf-8").read()
    body = text[text.index("enum class ModelId"):]
    found = {}
    for m in re.finditer(r"^\s*k(\w+)\s*=\s*(0x[0-9A-Fa-f]+|\d+)", body, re.M):
        value = int(m.group(2), 0)
        found.setdefault(value, pretty(m.group(1)))
    out = ["//! The names of the models of the characters of the overworld, by the number of the model.\n//!\n//! Written by `tools/gen_character_names.py` from the plugin: do not change it by hand.\n\n"]
    out.append("pub const NAMES: &[(u16, &str)] = &[\n")
    for value in sorted(found):
        out.append('    (%d, "%s"),\n' % (value, found[value].replace('"', "'")))
    out.append("];\n")
    open(OUT, "w", encoding="utf-8", newline="\n").write("".join(out))
    print(len(found), "names")


main()
