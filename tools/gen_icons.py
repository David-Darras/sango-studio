"""Writes module/species/src/icons.rs: the icon file of each species.

The table comes from the plugin: lib/src/ui/widget/icon.cc has the array
kPokemonIconFiles. The index of the array is the species number, and the
value is the file of the icon (the male, form 0) in the archive of the
Pokemon icons.

  python tools/gen_icons.py <path of the plugin repository>

The path can also come from the environment variable SANGO_PLUGIN.
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SOURCE = os.path.join("lib", "src", "ui", "widget", "icon.cc")
OUT = os.path.join(REPO, "module", "species", "src", "icons.rs")

ARRAY = "kPokemonIconFiles"
PER_LINE = 10


def plugin_path():
    """The path of the plugin repository, from the command line or the environment."""
    if len(sys.argv) > 1:
        return sys.argv[1]
    path = os.environ.get("SANGO_PLUGIN")
    if path:
        return path
    sys.exit("Give the path of the plugin repository: python tools/gen_icons.py <path>")


def read_table(path):
    """Returns the values of the array as a list of integers."""
    with open(path, encoding="utf-8") as f:
        text = f.read()
    match = re.search(r"const u16 %s\[(\d+)\] = \{([^}]*)\};" % ARRAY, text)
    if not match:
        sys.exit("The array %s is not in %s." % (ARRAY, path))
    size = int(match.group(1))
    values = [int(v) for v in re.findall(r"\d+", match.group(2))]
    if len(values) != size:
        sys.exit("The array %s has %d values, not %d." % (ARRAY, len(values), size))
    return values


def main():
    source = os.path.join(plugin_path(), SOURCE)
    values = read_table(source)
    out = [
        "//! The file of the Pokemon icon of each species.",
        "//!",
        "//! Written by `tools/gen_icons.py` from the plugin: do not change it by hand.",
        "",
        "/// The number of the archive that has the Pokemon icons.",
        "pub const ICON_ARCHIVE: usize = 91;",
        "",
        "/// The icon file of each species in the archive of the Pokemon icons.",
        "///",
        "/// The index is the species number; the index 0 is not used. The file is",
        "/// the icon of the male, form 0.",
        "pub const ICON_FILES: [u16; %d] = [" % len(values),
    ]
    for start in range(0, len(values), PER_LINE):
        line = ", ".join(str(v) for v in values[start:start + PER_LINE])
        out.append("    %s," % line)
    out.append("];")
    out.append("")
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out))
    print("icon files: %d" % len(values))


if __name__ == "__main__":
    main()
