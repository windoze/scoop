"""Select the native input and binding section of an explicit link-plan snapshot."""

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
    # Keep the entire section, including unfamiliar native fields and ordering.
    # Core object counts, startup references and generated C belong to other tests.
    return data[data.index(start) + 1 : data.index(end)] + b"\n"
