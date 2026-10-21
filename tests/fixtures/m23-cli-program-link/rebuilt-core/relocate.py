"""Relocate the actual String definition without nesting its documentation comments."""

import sys
from pathlib import Path

directory, prefix = map(Path, sys.argv[1:])
source = (directory / "strings.scoop").read_text()
boundary = '@Extern(name = "scoop_rt_string_eq", abi = "scoop")'
assert source.count(boundary) == 1
definition, helpers = source.split(boundary)
body = definition.index("{", definition.index("public class String ")) + 1
definition = definition[:body] + "\n    public fun rebuiltAnswer(): Int = 42\n" + definition[body:]
(directory / "strings.scoop").write_text(
    "public typealias String = relocated.String\n\n" + boundary + helpers
)
(directory / "relocated-string.scoop").write_text(prefix.read_text() + definition)
