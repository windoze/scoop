"""Independent protocol peer; compiler orchestration stays in fixture steps."""

import json
import os
import sys
from pathlib import Path

MODE = "normal"


def read_message():
    length = None
    while True:
        line = sys.stdin.buffer.readline()
        if line == b"\r\n":
            break
        if not line:
            raise AssertionError("incomplete protocol header")
        name, value = line.decode("ascii").split(":", 1)
        if name.lower() == "content-length":
            length = int(value.strip())
    assert length is not None
    parts = []
    while length:
        part = sys.stdin.buffer.read(min(length, 17))
        assert part
        parts.append(part)
        length -= len(part)
    return json.loads(b"".join(parts))


def write_bytes(data):
    for offset in range(0, len(data), 7):
        part = memoryview(data)[offset : offset + 7]
        while part:
            part = part[os.write(sys.stdout.fileno(), part) :]


def send_message(message):
    payload = json.dumps({"jsonrpc": "2.0", **message}, ensure_ascii=False).encode("utf-8")
    write_bytes(
        f"Content-Length: {len(payload)}\r\nContent-Type: application/json\r\n\r\n".encode()
        + payload
    )


def query(method, identifier, kind, **fields):
    send_message(
        {
            "id": identifier,
            "method": "compiler/query",
            "params": {"context": method, "query": {"kind": kind, **fields}},
        }
    )
    result = read_message()
    assert result["id"] == identifier
    return result


def inspect(task, method):
    declaration = method["method"]
    signature = method["signature"]
    assert signature["method"] == declaration
    assert method["requirements"]
    assert all(len(requirement["slot"]) == 64 for requirement in method["requirements"])
    send_message({"id": "unknown", "method": "not/a/compiler/method", "params": {}})
    assert read_message()["error"]["code"] == -32601
    send_message({"id": "bad-params", "method": "compiler/query", "params": {"context": 0}})
    assert read_message()["error"]["code"] == -32602
    missing = {"kind": "function", "id": "0" * 64}
    assert query(missing, "wrong-context", "describeTypes", types=[])["error"]["code"] == -32602
    unknown_type = {"kind": "nominal", "declaration": "0" * 64}
    failure = query(declaration, "bad-type", "describeTypes", types=[unknown_type])
    assert failure["error"]["code"] == -32000
    assert failure["error"]["data"]["diagnostics"]
    assert (
        len(
            query(declaration, "type", "describeTypes", types=[signature["result"]])["result"][
                "types"
            ]
        )
        == 1
    )
    assert (
        len(
            query(declaration, "members", "describeMembers", types=[task["owner"]])["result"][
                "members"
            ]
        )
        == 1
    )
    body = query(declaration, "body", "describeBodies", callables=[declaration])
    assert body["result"]["bodies"][0] == {"availability": "pendingGeneration"}


def generated_source(method):
    name = method["signature"]["name"]
    if name == "add":
        return "{\n    val result = left + right\n    return result\n}"
    if name == "label":
        return '// 中文前缀\n{ return "中文\\n\\"quoted\\"" } /* suffix */'
    if name == "echo":
        return "{ return value }"
    assert name == "value"
    return "{ return 42 }"


def main():
    request = read_message()
    assert request["method"] == "generator/generate"
    assert isinstance(request["id"], str)
    params = request["params"]
    assert params["protocolVersion"] == 1
    methods = [(task, method) for task in params["tasks"] for method in task["methods"]]
    assert methods
    if MODE in {"cache", "fragment-warning"}:
        with Path(__file__).with_suffix(".calls").open("a", encoding="utf-8") as output:
            output.write("called\n")
    if MODE == "cache":
        inspect(*methods[0])
    elif MODE == "logged":
        print("generator log", file=sys.stderr)
    if MODE == "normal":
        assert len(params["tasks"]) == 2
        assert len(params["tasks"][0]["bindings"]) == 3
        assert len(methods) == 3
        inspect(*methods[0])
        print("generator: 1 request, 2 owners, 3 methods, queries completed", file=sys.stderr)
    if MODE == "eof":
        return 0
    if MODE == "truncated":
        write_bytes(b"Content-Length: 20\r\n\r\n{")
        return 0
    if MODE == "bad-json":
        write_bytes(b"Content-Length: 1\r\n\r\n{")
        return 0
    if MODE == "version":
        send_message(
            {
                "id": request["id"],
                "error": {"code": -32602, "message": "unsupported generator protocol version"},
            }
        )
        return 0
    bodies = [
        {"method": method["method"], "source": generated_source(method)} for _, method in methods
    ]
    if MODE == "partial":
        bodies = [
            body
            for body, (_, method) in zip(bodies, methods, strict=True)
            if method["signature"]["name"] == "value"
        ]
    elif MODE == "empty":
        bodies = []
    elif MODE == "duplicate":
        bodies.append(bodies[0])
    elif MODE == "foreign":
        bodies[0]["method"] = {"kind": "function", "id": "0" * 64}
    elif MODE == "syntax":
        bodies[0]["source"] = "{\n    val 中文 = )\n    return 42\n}"
    elif MODE == "trailing":
        bodies[0]["source"] += "\nfun injected(): Int = 1"
    elif MODE == "type":
        bodies[0]["source"] = '{\n    return "中文"\n}'
    elif MODE == "unknown-name":
        bodies[0]["source"] = "{\n    return missingName\n}"
    diagnostics = []
    if MODE == "fragment-warning":
        bodies[0]["source"] = "// 中文\n{\n    return 42\n}"
        method = bodies[0]["method"]
        start = bodies[0]["source"].encode("utf-8").index(b"return")
        diagnostics.append(
            {
                "severity": "warning",
                "message": "fragment warning",
                "location": {
                    "source": {
                        "cone": params["currentCone"],
                        "path": f"generated/{method['kind']}/{method['id']}.scoop",
                    },
                    "span": {"startByte": start, "endByte": start + 6},
                },
                "notes": [],
            }
        )
    elif MODE in {"warning", "error"}:
        diagnostics.append(
            {"severity": MODE, "message": "peer diagnostic", "location": None, "notes": []}
        )
    elif MODE in {"located-error", "invalid-range"}:
        origin = methods[0][1]["origin"]
        diagnostics.append(
            {
                "severity": "error",
                "message": "peer diagnostic",
                "location": {
                    "source": origin["source"],
                    "span": {
                        "startByte": 10 if MODE == "invalid-range" else 0,
                        "endByte": 9,
                    },
                },
                "notes": [{"message": "peer note", "location": origin}],
            }
        )
    response = {"id": request["id"], "result": {"bodies": bodies, "diagnostics": diagnostics}}
    if MODE == "wrong-id":
        response["id"] = "wrong"
    send_message(response)
    if MODE == "extra":
        write_bytes(b"extra")
    if MODE == "exit-failure":
        return 7
    return 0


if __name__ == "__main__":
    sys.exit(main())
