"""Compare actual root machine code and link output from the fixed toolsets."""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


def inspect(case, target):
    binary = Path(case["binary"])
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    if digest != case["binary_sha256"]:
        raise RuntimeError("binary changed after build")
    records = [json.loads(line) for line in case["stderr"].splitlines()]
    result = records[-1]
    root = result["root"]["identity"]
    observation = next(node for node in result["observations"]["nodes"] if node["identity"] == root)
    if (
        observation["origin"] != "compiled"
        or root not in result["observations"]["child_invocations"]
    ):
        raise RuntimeError("root must be freshly compiled by its selected toolset")
    lir = (Path(case["dump"]) / root / "lir.txt").read_text()
    symbols = re.findall(r"^  fun @([^ (]+)\(", lir, re.M)
    prefix = "_" if target == "aarch64-apple-darwin" else ""
    command = [
        "llvm-objdump",
        "--disassemble-symbols=" + ",".join(prefix + s for s in symbols),
        str(binary),
    ]
    assembly = subprocess.check_output(command, text=True)
    if not symbols or any(f"<{prefix}{symbol}>:" not in assembly for symbol in symbols):
        raise RuntimeError("missing root function in disassembly")
    instructions = re.findall(
        r"^\s*[0-9a-f]+:\s+((?:(?:[0-9a-f]{8}|[0-9a-f]{2})\s+)+)", assembly, re.M
    )
    if not instructions:
        raise RuntimeError("no decoded instructions")
    machine = "".join("".join(part.split()) for part in instructions)
    return {
        "label": case["label"],
        "binary_sha256": digest,
        "binary_bytes": binary.stat().st_size,
        "cache_key": observation["cache_key"],
        "root_artifact": result["root"]["artifact_fingerprint"],
        "lir_sha256": hashlib.sha256(lir.encode()).hexdigest(),
        "symbols": symbols,
        "machine_bytes": len(machine) // 2,
        "instruction_count": len(instructions),
        "machine_sha256": hashlib.sha256(machine.encode()).hexdigest(),
        "command": command,
        "assembly": assembly,
        "sections": subprocess.check_output(["llvm-size", "--format=sysv", str(binary)], text=True),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists")
    build = json.loads(args.build.read_text())
    report = {"target": build["target"], "tools": build["tools"], "cases": {}}
    for case in build["cases"]:
        report["cases"].setdefault(case["name"], []).append(inspect(case, build["target"]))
    for name, pair in report["cases"].items():
        if len(pair) != 2 or pair[0]["cache_key"] == pair[1]["cache_key"]:
            raise RuntimeError("expected separately compiled baseline and candidate")
        for key in (
            "binary_sha256",
            "binary_bytes",
            "root_artifact",
            "lir_sha256",
            "symbols",
            "machine_bytes",
            "instruction_count",
            "machine_sha256",
        ):
            if pair[0][key] != pair[1][key]:
                raise RuntimeError(
                    f"{name}: changed {key}; inspect before accepting the boundary change"
                )
        print(name, pair[0]["machine_bytes"], "root code bytes unchanged", flush=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
