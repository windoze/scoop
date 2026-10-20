"""Select the native input and binding section of an explicit link-plan snapshot."""

import re

from .model import AssertionFailure


def native_link_plan(data):
    start = b"\nlink inputs:"
    end = b"\nstartup object references:\n"
    if (
        not data.startswith(b"program-link v3\n")
        or data.count(start) != 1
        or data.count(end) != 1
        or data.index(start) >= data.index(end)
    ):
        raise AssertionFailure("native-link-plan requires a complete program-link v3 dump")
    # Preserve native inputs, bindings, ordering and unfamiliar fields. The
    # whole-program import total also includes unrelated runtime/backend calls.
    section = data[data.index(start) + 1 : data.index(end)] + b"\n"
    return b"".join(
        line
        for line in section.splitlines(keepends=True)
        if not re.fullmatch(rb"ELF system imports=[0-9]+\n", line)
    )
