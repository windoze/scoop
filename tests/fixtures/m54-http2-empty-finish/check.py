import os
import subprocess
import sys


def frames(wire):
    if wire.startswith(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"):
        wire = wire[24:]
    result = []
    while wire:
        assert len(wire) >= 9
        count = int.from_bytes(wire[:3], "big")
        assert len(wire) >= count + 9
        kind, flags = wire[3:5]
        stream = int.from_bytes(wire[5:9], "big")
        assert kind in (0, 1, 4), (kind, flags, stream)
        result.append((kind, flags, stream, wire[9 : 9 + count]))
        wire = wire[9 + count :]
    return result


def check(mode, trace):
    phases = {name: frames(bytes.fromhex(encoded)) for name, encoded in trace}
    assert len(phases) == len(trace)
    all_frames = [frame for phase in phases.values() for frame in phase]
    for name in ("head", "body", "blocked"):
        assert all(not flags & 1 for kind, flags, _, _ in phases.get(name, []) if kind != 4)
    streams = (1, 3) if mode == "connection" else (1,)
    for stream in streams:
        messages = [frame for frame in all_frames if frame[2] == stream]
        heads = [frame for frame in messages if frame[0] == 1]
        assert len(heads) == (3 if mode == "response" else 2), (mode, heads)
        assert sum(bool(flags & 1) for _, flags, _, _ in messages) == 1
        assert messages[-1][0] == 1 and messages[-1][1] & 5 == 5
        expected_size = {"exact": 3, "blocked": 4, "connection": 65535}.get(mode, 0)
        if stream == 3:
            expected_size = 0
        body = b"".join(payload for kind, _, _, payload in messages if kind == 0)
        assert body == b"x" * expected_size
        assert bool(messages[-1][3]) == (mode in ("exact", "blocked"))
    if mode == "blocked":
        assert sum(len(payload) for kind, _, _, payload in phases["blocked"] if kind == 0) == 3
        assert sum(len(payload) for kind, _, _, payload in phases["end"] if kind == 0) == 1


for mode in ("early", "late", "exact", "blocked", "connection", "response"):
    result = subprocess.run(
        [sys.argv[1], mode],
        env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        capture_output=True,
        text=True,
        timeout=180,
    )
    assert result.returncode == 0 and not result.stderr, (mode, result.returncode, result.stderr)
    traces = []
    current = None
    for line in result.stdout.splitlines():
        if line == "begin":
            assert current is None
            current = []
        elif line == "done":
            assert current is not None
            traces.append(current)
            current = None
        else:
            assert current is not None
            current.append(line.split(":", 1))
    assert current is None and len(traces) == 2
    for trace in traces:
        check(mode, trace)
print("HTTP2 empty finishes preserve flow control and frame order")
