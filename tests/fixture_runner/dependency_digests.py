"""Normalize only the six semantic content digests in stale-dependency diagnostics."""

import re

HEX = rb"[0-9a-f]{64}"
STALE = re.compile(
    rb"(StaleDependency: Cone "
    + HEX
    + rb" requires [^;\n]+ \("
    + HEX
    + rb"\); recorded HIR/MIR/LIR )("
    + HEX
    + rb")/("
    + HEX
    + rb")/("
    + HEX
    + rb"), actual ("
    + HEX
    + rb")/("
    + HEX
    + rb")/("
    + HEX
    + rb")(?=[\"\n]|$)"
)


def normalize_dependency_digests(data):
    tokens = {}

    def replace(match):
        values = [
            tokens.setdefault(value, f"$DEPENDENCY_DIGEST_{len(tokens)}".encode())
            for value in match.groups()[1:]
        ]
        return match[1] + b"/".join(values[:3]) + b", actual " + b"/".join(values[3:])

    return STALE.sub(replace, data)
