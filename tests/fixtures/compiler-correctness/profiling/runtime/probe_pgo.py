import base64
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

scoop, pgo, merger, source, directory, cache, sysroot, *native = sys.argv[1:]
work = Path(directory)
data = work / "pgo 数据 %"
local = work / "training program.scoop"
shutil.copyfile(source, local)
original = local.read_bytes()
base = [pgo, "--scoop", scoop, "--sysroot", sysroot, "--llvm-profdata", merger]


def call(command, *args, expected=0, stdout=b"", cwd=work, data_dir=data):
    result = subprocess.run(
        [*base, command, "--data-dir", str(data_dir), *map(str, args)],
        cwd=cwd,
        capture_output=True,
        timeout=180,
    )
    assert result.returncode == expected, result
    assert result.stdout == stdout, result
    return result


def read(path):
    return json.loads(path.read_text())


def path(value):
    value_bytes = base64.b64decode(value["bytes"])
    if value["encoding"] == "UnixBytes":
        return Path(os.fsdecode(value_bytes))
    return Path(value_bytes.decode("utf-16-le", errors="surrogatepass"))


def entry_count(name):
    bodies = read(data / "merged/summary.json")["bodies"]
    return next(body["positions"][0]["count"] for body in bodies if body["name"] == name)


call(
    "instrument",
    local.name,
    *native,
    "--cache-dir",
    cache,
    "--target-dir",
    "pgo build outputs",
    "--library-path",
    ".",
    stdout=b"15\n",
)
saved = read(data / "build.json")
options = saved["invocation"]["options"]
assert options["export_ir"] == "full" and options["profile"] == "profiling", saved
assert saved["invocation"]["effective_debug_info"] == "line-tables", saved
assert path(options["root_input"]).samefile(local)
assert path(options["library_path"][0]).samefile(work)
assert saved["maps"] and all((data / mapping).is_file() for mapping in saved["maps"])
assert entry_count("main") == 1 and entry_count("notObserved") == 0
instrumented = path(saved["output"])

# Append a business failure while preserving empty arguments and collecting its counters.
before = set((data / "runs").iterdir())
call("instrument", "--", "", "two words", expected=7, stdout=b"15\n10\n")
latest = (set((data / "runs").iterdir()) - before).pop()
run = read(latest / "run.json")
assert run["arguments"][0] is None and run["status"]["code"] == 7, run
assert entry_count("main") == 2 and entry_count("notObserved") == 3

# A different cwd must not change the selected source or relative build inputs.
other = work / "pgo other cwd"
other.mkdir()
call("optimize", "--emit", "optimizations", "--dump-dir", "decisions", cwd=other)
optimization = read(data / "optimization.json")
assert optimization["invocation"]["effective_debug_info"] == "none", optimization
assert any(cone["summary"]["consumers"] for cone in optimization["profile_use"]["source_cones"])
optimized = path(optimization["output"])
probe_directory = work / "no PGO probes"
result = subprocess.run(
    [optimized],
    capture_output=True,
    env=dict(os.environ, SCOOP_PROFILE_DIR=str(probe_directory)),
    timeout=30,
)
assert result.returncode == 0 and result.stdout == b"15\n" and result.stderr == b"", result
assert not probe_directory.exists()
merged_time = (data / "merged/counts.profdata").stat().st_mtime_ns
call("optimize", cwd=other)
assert read(data / "optimization.json")["observations"]["child_invocations"] == []
assert (data / "merged/counts.profdata").stat().st_mtime_ns == merged_time

# Build-only operation does not invent a run; external files are collected by optimize.
before = set((data / "runs").iterdir())
call("instrument", "--no-run")
pending = (set((data / "runs").iterdir()) - before).pop()
assert read(pending / "run.json")["status"] is None
assert not list(pending.rglob("*.profraw"))
result = subprocess.run(
    [instrumented],
    capture_output=True,
    env=dict(os.environ, SCOOP_PROFILE_DIR=str(pending)),
    timeout=30,
)
assert result.returncode == 0 and result.stdout == b"15\n", result
explicit = work / "final binary"
call("optimize", "-o", explicit)
assert path(read(data / "optimization.json")["output"]) == explicit
assert entry_count("main") == 3

# Ordinary build and merge failures preserve the previously published outputs.
previous_binary = instrumented.read_bytes()
previous_build = (data / "build.json").read_bytes()
local.write_text("fun main(\n")
try:
    call("instrument", expected=1)
    assert instrumented.read_bytes() == previous_binary
    assert (data / "build.json").read_bytes() == previous_build
finally:
    local.write_bytes(original)
result = call("instrument", "--export-ir", "default", expected=1)
assert b"different root, target, export policy" in result.stderr, result
assert instrumented.read_bytes() == previous_binary
assert (data / "build.json").read_bytes() == previous_build
result = call("instrument", "--release", expected=1)
assert b"managed by scoop-pgo" in result.stderr

previous_output = explicit.read_bytes()
previous_merged = (data / "merged/counts.profdata").read_bytes()
raw = next(pending.rglob("*.profraw"))
raw_bytes = raw.read_bytes()
raw.write_bytes(b"invalid runtime profile")
try:
    call("optimize", "-o", explicit, expected=1)
    assert explicit.read_bytes() == previous_output
    assert (data / "merged/counts.profdata").read_bytes() == previous_merged
finally:
    raw.write_bytes(raw_bytes)
call("optimize", "-o", explicit)
assert explicit.is_file()
run_path = pending / "run.json"
run_record = run_path.read_bytes()
missing = read(run_path)
missing["raw"].append("missing.profraw")
run_path.write_text(json.dumps(missing))
try:
    result = call("optimize", "-o", explicit, expected=1)
    assert b"training input is missing" in result.stderr, result
    assert explicit.read_bytes() == previous_output
finally:
    run_path.write_bytes(run_record)
# Workspace selection is saved as the actual Cone, even if defaults change later.
project = work / "pgo workspace"
project.mkdir()
manifest = project / "Cone.toml"
manifest.write_text(
    'schema = 1\n[workspace]\nmembers = ["one", "two"]\ndefault-members = ["one"]\n'
)
for name, value in [("one", 4), ("two", 8)]:
    cone = project / name
    (cone / "src").mkdir(parents=True)
    (cone / "Cone.toml").write_text(
        f'schema = 1\n[cone]\ngroup = "example.pgo"\nname = "{name}"\n'
        'version = "0.1.0"\nkind = "executable"\n[dependencies]\n"scoop:scoop.std" = "0.1.0"\n'
    )
    (cone / "src/main.scoop").write_text(
        f"package example.pgo\nfun helper(value: Int): Int {{ return value + 1 }}\nfun main() {{ println(helper({value})) }}\n"
    )
workspace_data = work / "pgo workspace data"
call("instrument", project, *native, "--cache-dir", cache, "--no-run", data_dir=workspace_data)
result = call("optimize", data_dir=workspace_data, expected=1)
assert b"no runtime counters" in result.stderr, result
selected = read(workspace_data / "build.json")
assert path(selected["invocation"]["options"]["root_input"]).samefile(project / "one/Cone.toml")
manifest.write_text(
    manifest.read_text().replace('default-members = ["one"]', 'default-members = ["two"]')
)
call("instrument", data_dir=workspace_data, cwd=project, stdout=b"5\n")
call("optimize", data_dir=workspace_data, cwd=other)
selected = read(workspace_data / "optimization.json")
assert path(selected["invocation"]["options"]["root_input"]).samefile(project / "one/Cone.toml")
sys.stdout.buffer.write(b"profiling command passed\n")
