"""Normalize explicit native content digests while retaining their references."""

import re

HEX = rb"[0-9a-f]{64}"
BYTE = rb"(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])"
FIELDS = re.compile(
    rb"^native library [^\n]+ requirement="
    + HEX
    + rb" input=(?P<input>"
    + HEX
    + rb")(?= origins=)"
    + rb"|\bnative (?:archive|object) (?P<object>"
    + HEX
    + rb")(?= members=\d| slice=\d|-member-\d+-\d+| selected by | section )"
    + rb"|^  member \d+ header=\d+ payload=\d+\.\.\d+ name=[^\n]+ digest=(?P<member>"
    + HEX
    + rb")(?= selected=(?:true|false)(?:\n|$))"
    + rb"|^dynamic provider (?P<provider>"
    + HEX
    + rb")(?= install=)"
    + rb"|^  (?:load|re-export) [^\n]+ -> (?P<edge>"
    + HEX
    + rb")(?=\n|$)"
    + rb"|NativeInputId\(Digest256\(\[(?P<debug>(?:"
    + BYTE
    + rb", ){31}"
    + BYTE
    + rb")\]\)\)",
    re.MULTILINE,
)


def normalize_native_digests(data):
    tokens = {"input": {}, "member": {}, "provider": {}}

    def replace(match):
        field = match.lastgroup
        value = match.group(field)
        if field == "debug":
            value = bytes(int(part) for part in value.split(b", ")).hex().encode()
        kind = (
            "input"
            if field in {"input", "object", "debug"}
            else "provider"
            if field in {"provider", "edge"}
            else "member"
        )
        names = tokens[kind]
        token = names.setdefault(value, f"$NATIVE_{kind.upper()}_{len(names)}".encode())
        start, end = match.span(field)
        return data[match.start() : start] + token + data[end : match.end()]

    return FIELDS.sub(replace, data)
