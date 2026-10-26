"""Check fusion and the retained shared intermediate in complete typed IR."""

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
for name, original_calls in (("fused", 4), ("nested", 3), ("shared", 2)):
    function = functions[name]
    calls = function.count("@scoop_rt_string_concat direct")
    if release and name != "shared":
        assert calls == 0 and function.count("StringConcat\n") == 1, function
    else:
        assert calls == original_calls and "StringConcat\n" not in function, function
if release:
    assert 'Utf8 " / ["' in functions["fused"]
    assert " = string_concat " in (directory / "lir.txt").read_text()
    assert functions["binaryLoop"].count("StringConcat\n") == 1
    assert "@scoop_rt_string_concat direct" not in functions["binaryLoop"]
else:
    assert functions["binaryLoop"].count("@scoop_rt_string_concat direct") == 1
