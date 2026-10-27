"""Check the short path, fusion boundaries, and the ordinary long fallback."""

import re
import sys
from pathlib import Path

directory = Path(sys.argv[1])
release = sys.argv[2] == "release"
source = (directory / "mir.txt").read_text()
functions = {
    match[1]: match.group()
    for match in re.finditer(r"^  fun (\S+) @fn\d+.*?(?=^  fun |\Z)", source, re.M | re.S)
}
for name, count in (
    ("three", 2),
    ("eight", 7),
    ("branch", 4),
    ("longText", 2),
    ("Suspension.run$drive", 4),
    ("DefaultSuspension.run$drive", 2),
):
    function = functions[name]
    assert "ClassAlloc StringBuilder" not in function, function
    assert "scoop_rt_string_join_parts" not in function, function
    if release:
        assert function.count("StringConcat\n") == 1, function
        assert "@scoop_rt_string_concat direct" not in function, function
    else:
        assert function.count("@scoop_rt_string_concat direct") == count, function
        assert "StringConcat\n" not in function, function
assert functions["two"].count("@scoop_rt_string_concat direct") == 1
assert functions["nine"].count("ClassAlloc StringBuilder") == 1
