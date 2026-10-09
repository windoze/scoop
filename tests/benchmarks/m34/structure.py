import argparse
import json
import platform
import re
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("kind", choices=["interfaces", "poll"])
args = parser.parse_args()
repo = Path.cwd()
host = "linux-gnu" if platform.system() == "Linux" else "darwin"
for mode in ["off", "on"]:
    work = repo / f"tmp/m34/final-{args.kind}-{mode}-{host}"
    binary = work / args.kind
    dumps = work / ("provider-dump" if args.kind == "interfaces" else "dump")
    dump = next(dumps.iterdir())
    mir = re.findall(r"^  fun (\S+) @fn(\d+)\(", (dump / "mir.txt").read_text(), re.M)
    lir = re.findall(r"^  fun @([^ (]+)\(", (dump / "lir.txt").read_text(), re.M)
    selected = (
        ["Cell.next", "Cell.$get$salt", "known", "unknown", "convertedOnce", "convertedEach"]
        if args.kind == "interfaces"
        else ["compute"]
    )
    if args.kind == "interfaces":
        assert [name for name, _ in mir[:6]] == selected
    output, mapping = [], []
    for (name, identifier), symbol in zip(mir, lir, strict=False):
        if name not in selected:
            continue
        native = ("_" if host == "darwin" else "") + symbol
        assembly = subprocess.check_output(
            ["llvm-objdump", f"--disassemble-symbols={native}", str(binary)], text=True
        )
        assert native in assembly and len(assembly.splitlines()) > 10, native
        instructions = re.findall(
            r"^\s*[0-9a-f]+:\s+((?:(?:[0-9a-f]{8}|[0-9a-f]{2})\s+)+)", assembly, re.M
        )
        size = sum(len("".join(group.split())) // 2 for group in instructions)
        mapping.append(
            {
                "source": name,
                "mir_function": identifier,
                "symbol": symbol,
                "machine_bytes": size,
                "instruction_count": len(instructions),
            }
        )
        output.append(f"; {name}, MIR fn{identifier}, LIR {symbol}\n" + assembly)
    assert sorted(item["source"] for item in mapping) == sorted(selected)
    report = json.loads((work / "build.json").read_text())
    report["sections"] = subprocess.check_output(
        ["llvm-size", "--format=sysv", str(binary)], text=True
    )
    report["hot_functions"] = mapping
    destination = repo / f"tmp/m34/final-{args.kind}-{mode}-{host}"
    destination.with_suffix(".asm").write_text("\n".join(output))
    destination.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n")
    print(mode, [(row["source"], row["machine_bytes"]) for row in mapping])
