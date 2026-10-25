"""Check typed constant reads; dynamic Unicode decoding retains its leaf fallback."""

import re
import sys
from pathlib import Path

source = (Path(sys.argv[1]) / "mir.txt").read_text()
functions = {
    m[1]: m.group()
    for m in re.finditer(r"^  fun (\S+) @fn\d+.*?(?=^  fun |\Z)", source, re.M | re.S)
}
if sys.argv[2] == "release":
    assert "@scoop_rt_string_compare" not in functions["constantReads"], functions["constantReads"]
    assert "@scoop_rt_string_byte_length" not in functions["constantReads"], functions[
        "constantReads"
    ]
    assert not re.search(r"^\s+call ", functions["constantHash"], re.M), functions["constantHash"]
