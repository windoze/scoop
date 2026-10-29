"""Annotate exact Scoop symbols in native sample/perf text without regrouping it."""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

LOGICAL_SYMBOL = r"scoop\$1\$cb\$[0-9a-f]{64}"
SYMBOL = re.compile(r"(?<![\w$.])_?scoop\$1\$(?:cb|bs)\$[0-9a-f]{64}(?![\w$.])")


def labels(paths):
    result = {}
    for path in paths:
        report = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(report, dict) or report.get("schema") != 1:
            raise ValueError(f"{path}: expected symbols schema 1")
        entries = report.get("symbols")
        if not isinstance(entries, list):
            raise ValueError(f"{path}: expected a symbols array")
        for entry in entries:
            if not isinstance(entry, dict):
                raise ValueError(f"{path}: expected a symbol record")
            symbol, name, role = (
                entry.get("symbol"),
                entry.get("name"),
                entry.get("role"),
            )
            if (
                not isinstance(symbol, str)
                or re.fullmatch(LOGICAL_SYMBOL, symbol) is None
                or not isinstance(role, str)
                or (name is not None and not isinstance(name, str))
            ):
                raise ValueError(f"{path}: invalid symbol, name or role")
            label = name or f"<{role}>"
            if name and role != "callable":
                label += f" [{role}]"
            label = " ".join(label.splitlines())
            if symbol in result and result[symbol] != label:
                raise ValueError(f"{path}: conflicting labels for {symbol}")
            result[symbol] = label
    return result


def annotate(text, names):
    def replace(match):
        original = match.group()
        logical = original.removeprefix("_")
        label = names.get(logical)
        return f"{label} {{{original}}}" if label is not None else original

    return SYMBOL.sub(replace, text)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--symbols", type=Path, action="append", required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--input", type=Path, help="Native text report")
    source.add_argument(
        "--address",
        type=lambda value: int(value, 0),
        action="append",
        help="PC to resolve with --binary; may be repeated",
    )
    parser.add_argument("--output", type=Path, help="Annotated text; default stdout")
    parser.add_argument(
        "--binary", type=Path, help="Sampled executable; resolve real symbol aliases"
    )
    parser.add_argument(
        "--llvm-tools", type=Path, help="External LLVM bin directory (upstream command names)"
    )
    parser.add_argument(
        "--load-bias",
        type=lambda value: int(value, 0),
        default=0,
        help="Actual image load bias for --address; defaults to linked addresses",
    )
    args = parser.parse_args(argv)
    try:
        inputs = {path.resolve() for path in args.symbols}
        if args.input:
            inputs.add(args.input.resolve())
        if args.binary:
            inputs.add(args.binary.resolve())
        if args.output and args.output.resolve() in inputs:
            raise ValueError("output must not overwrite the report, symbols or sampled binary")
        names = labels(args.symbols)
        if args.binary and not args.address:
            import binary_symbols

            names = binary_symbols.load(names, args.binary, args.llvm_tools)
        if args.address:
            if args.binary is None:
                raise ValueError("--address requires --binary")
            import dwarf_symbols

            frames = dwarf_symbols.resolve(
                args.binary, args.address, names, args.llvm_tools, args.load_bias
            )
            output = "".join(
                f"0x{address:x} = {frames.get(address, '<unresolved>')}\n"
                for address in args.address
            )
        else:
            raw = args.input.read_text(encoding="utf-8")
            output = annotate(raw, names)
            if args.binary and raw.startswith("Analysis of sampling "):
                import sample_symbols

                output = sample_symbols.annotate(output, names, args.binary, args.llvm_tools)
        if args.output:
            args.output.write_text(output, encoding="utf-8")
        else:
            sys.stdout.write(output)
    except subprocess.CalledProcessError as error:
        print(f"symbolize: {error}\n{error.stderr.strip()}", file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"symbolize: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
