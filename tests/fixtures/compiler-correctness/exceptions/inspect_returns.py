"""Keep return/finally control flow independent of unrelated imported functions."""

import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
names = {
    identity: name for name, identity in re.findall(r"^  fun (\S+) (@fn\d+)\(", text, re.MULTILINE)
}
selected = (
    "afterReturn",
    "fromIf",
    "fromWhen",
    "fromFinally",
    "exceptionReplacedByFinally",
    "branchingFinally",
)
output = []
for name in selected:
    body = re.search(rf"^  fun {name} @fn\d+\(.*?(?=^  fun |\Z)", text, re.MULTILINE | re.DOTALL)
    assert body, name
    output.append(name)
    for line in body.group().splitlines()[1:]:
        if re.match(
            r"\s+(bb\d+ |return$|unreachable$|goto |branch |throw |landing_pad |"
            r"begin_catch$|end_catch$|resume$|call |IntegerLiteral )",
            line,
        ):
            line = re.sub(r"@fn\d+", lambda match: "@" + names[match[0]], line)
            output.append(line[4:])
Path(sys.argv[2]).write_text("\n".join(output) + "\n")
