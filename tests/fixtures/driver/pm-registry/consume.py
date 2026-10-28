"""Compile packages served as ordinary files, without a publishing service."""

import functools
import hashlib
import http.server
import io
import json
import os
import subprocess
import sys
import tarfile
import threading
import tomllib
from pathlib import Path


def run(arguments):
    environment = {**os.environ, "NO_PROXY": "127.0.0.1", "no_proxy": "127.0.0.1"}
    result = subprocess.run(arguments, capture_output=True, text=True, env=environment)
    assert result.returncode == 0, (arguments, result.stdout, result.stderr)
    return result


def manifest(name, version, kind="library", dependencies=""):
    return (
        f'schema = 1\n[cone]\ngroup = "test"\nname = "{name}"\n'
        f'version = "{version}"\nkind = "{kind}"\n[dependencies]\n{dependencies}'
    )


def main():
    scoop, scoopc, pm, directory, sysroot, cone_cache, target, *compile_args = sys.argv[1:]
    work = Path(directory)
    site = work / "static"
    base = site / "repo"
    base.mkdir(parents=True)
    (base / "config.json").write_text('{"format":1}')
    requests = []

    class Handler(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            super().do_GET()

        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(Handler, directory=site)
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{server.server_port}/repo/"
    versions = {}

    def package(name, version, source, dependency=False, absolute=False):
        dependencies = [
            {
                "key": "scoop:scoop.core",
                "version": "=0.1.0",
                "source": {"kind": "sysroot"},
            }
        ]
        declaration = ""
        if dependency:
            declaration = f'"test:leaf" = {{version = "1.0.0", registry = "{url}"}}\n'
            dependencies.append(
                {
                    "key": "test:leaf",
                    "version": "1.0.0",
                    "source": {"kind": "registry", "registry": url},
                }
            )
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode="w") as archive:
            for path, text in {
                "Cone.toml": manifest(name, version, dependencies=declaration),
                "src/value.scoop": source,
            }.items():
                content = text.encode()
                info = tarfile.TarInfo(path)
                info.size = len(content)
                archive.addfile(info, io.BytesIO(content))
        # Test input preparation only; production publishing is a later milestone.
        archive = subprocess.run(
            ["zstd", "-q", "-c"], input=data.getvalue(), capture_output=True, check=True
        ).stdout
        relative = f"files/{name}/{version}.tar.zst"
        destination = base / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(archive)
        versions.setdefault(name, []).append(
            {
                "version": version,
                "kind": "library",
                "dependencies": dependencies,
                "archive": url + relative if absolute else relative,
                "sha256": hashlib.sha256(archive).hexdigest(),
                "yanked": False,
            }
        )
        index = base / f"packages/test/{name}.json"
        index.parent.mkdir(parents=True, exist_ok=True)
        index.write_text(
            json.dumps({"format": 1, "group": "test", "name": name, "versions": versions[name]})
        )

    def leaf(version, value):
        package("leaf", version, f"package test.leaf\npublic fun value(): Int = {value}\n")

    def source(project, expected):
        (project / "src/main.scoop").write_text(
            "package test\nimport test.top.selected\n"
            f"fun main() {{ if (selected() != {expected}) throw IllegalStateException() }}\n"
        )

    def invoke(project, *options):
        return run(
            [
                scoop,
                "--package-manager",
                pm,
                "run",
                str(project),
                "--release",
                *compile_args,
                "--sysroot",
                sysroot,
                "--scoopc",
                scoopc,
                "--cache-dir",
                cone_cache,
                "--package-cache",
                str(work / "packages"),
                "--message-format",
                "json",
                *options,
            ]
        )

    def selected(project):
        lock = tomllib.loads((project / "Cone.lock").read_text())
        return next(
            p["coordinate"].split(":")[2]
            for p in lock["resolutions"][0]["nodes"]
            if p["coordinate"].startswith("test:leaf:")
        )

    try:
        leaf("1.0.0", 1)
        leaf("1.2.0", 2)
        package(
            "top",
            "1.0.0",
            "package test.top\nimport test.leaf.value\npublic fun selected(): Int = value()\n",
            True,
            True,
        )
        projects = [work / name for name in ("one", "two")]
        for project in projects:
            (project / "src").mkdir(parents=True)
            (project / "Cone.toml").write_text(
                manifest(
                    project.name,
                    "1.0.0",
                    "executable",
                    f'"test:top" = {{version = "1.0.0", registry = "{url}"}}\n',
                )
            )
            source(project, 2)
            assert not (project / "Cone.lock").exists()
            invoke(project)
            assert selected(project) == "1.2.0"
        assert requests.count("/repo/files/top/1.0.0.tar.zst") == 1, requests
        assert requests.count("/repo/files/leaf/1.2.0.tar.zst") == 1, requests
        assert "/repo/files/leaf/1.0.0.tar.zst" not in requests, requests
        count = len(requests)
        leaf("1.3.0", 3)
        invoke(projects[0])
        assert selected(projects[0]) == "1.2.0"
        assert len(requests) == count, requests[count:]
        run(
            [
                pm,
                "update",
                str(projects[0]),
                "--target",
                target,
                "--package",
                "test:leaf",
                "--sysroot",
                sysroot,
                "--package-cache-dir",
                str(work / "packages"),
            ]
        )
        assert selected(projects[0]) == "1.3.0"
        source(projects[0], 3)
        count = len(requests)
        for project in projects:
            result = invoke(project, "--frozen")
            assert not result.stdout, result.stdout
        assert len(requests) == count, requests[count:]
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
    print("static registry consumption passed")


if __name__ == "__main__":
    main()
