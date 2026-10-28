"""Workspace members consume separate locked graphs, including cached Git sources."""

import subprocess
import sys
from pathlib import Path

from peer import Peer


def write(root, relative, text):
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return path


def manifest(name, version="1.0.0"):
    return (
        f'schema = 1\n[cone]\ngroup = "test"\nname = "{name}"\n'
        f'version = "{version}"\nkind = "library"\n'
    )


def run(args):
    result = subprocess.run(args, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def main():
    server, package_manager, directory, sysroot, target = sys.argv[1:]
    work = Path(directory)
    root = work / "workspace"
    repository = work / "repository"
    write(repository, "Cone.toml", manifest("shared"))
    library = write(
        repository, "src/value.scoop", "package shared\npublic fun selected(): Boolean = true\n"
    )
    run(["git", "init", "--quiet", str(repository)])
    run(["git", "-C", str(repository), "add", "."])
    run(
        [
            "git",
            "-C",
            str(repository),
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "original",
        ]
    )
    # The root Cone is intentionally excluded from the build default selection.
    write(
        root,
        "Cone.toml",
        manifest("root")
        + '[workspace]\nmembers = ["other", "missing"]\ndefault-members = ["other"]\n'
        + f'[dependencies]\n"test:shared" = {{git = "{repository.as_uri()}"}}\n',
    )
    write(work, "path/Cone.toml", manifest("shared", "2.0.0"))
    write(work, "path/src/value.scoop", 'package shared\npublic fun selected(): String = "two"\n')
    write(
        root,
        "other/Cone.toml",
        manifest("other") + '[dependencies]\n"test:shared" = {path = "../../path"}\n',
    )
    write(root, "missing/Cone.toml", manifest("missing"))
    source_text = "import shared.selected\nfun use() { val item = selected(); item. }\n"
    source_paths = [write(root, f"{prefix}src/use.scoop", source_text) for prefix in ("", "other/")]
    missing = write(root, "missing/src/local.scoop", "fun localOnly(): Boolean = true\n")
    cache = work / "packages"
    for member in ("test:root", "test:other"):
        run(
            [
                package_manager,
                "resolve",
                str(root),
                "-p",
                member,
                "--target",
                target,
                "--sysroot",
                sysroot,
                "--package-cache-dir",
                str(cache),
            ]
        )
    lock = root / "Cone.lock"
    before = (lock.read_bytes(), lock.stat().st_mtime_ns)
    # Moving the branch must not change what the editor sees from the locked checkout.
    library.write_text('package shared\npublic fun selected(): String = "new branch"\n')
    run(["git", "-C", str(repository), "add", "."])
    run(
        [
            "git",
            "-C",
            str(repository),
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "advance",
        ]
    )
    peer = Peer(server, package_manager, sysroot, target, cache)
    try:
        peer.request(
            "initialize",
            {
                "processId": None,
                "workspaceFolders": [{"name": "workspace", "uri": root.as_uri()}],
                "initializationOptions": {
                    "target": target,
                    "sysroot": sysroot,
                    "packageManager": package_manager,
                    "packageCacheDir": str(cache),
                },
                "capabilities": {
                    "textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": True}}
                },
            },
        )
        peer.notify("initialized", {})
        for path in [*source_paths, missing]:
            peer.notify(
                "textDocument/didOpen",
                {
                    "textDocument": {
                        "uri": path.as_uri(),
                        "version": 1,
                        "languageId": "scoop",
                        "text": path.read_text(),
                    }
                },
            )
        for path, expected in zip(source_paths, ("Boolean", "String"), strict=True):
            hover = peer.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": path.as_uri()},
                    "position": {
                        "line": 1,
                        "character": source_text.splitlines()[1].rindex("item."),
                    },
                },
            )
            assert hover["contents"]["value"] == f"item: {expected}", (
                path,
                hover,
                peer.diagnostics,
            )
        symbols = peer.request(
            "textDocument/documentSymbol", {"textDocument": {"uri": missing.as_uri()}}
        )
        assert symbols[0]["name"] == "localOnly", symbols
        symbols = peer.request("workspace/symbol", {"query": "localOnly"})
        assert any(item["name"] == "localOnly" for item in symbols), symbols
        diagnostics = peer.diagnostics[(root / "missing/Cone.toml").as_uri()]["diagnostics"]
        assert any(
            item["code"] == "SCOOP_PROJECT" and "resolve" in item["message"] for item in diagnostics
        ), diagnostics
        assert (
            peer.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": missing.as_uri()},
                    "position": {"line": 0, "character": 5},
                },
            )
            is None
        )
        assert (lock.read_bytes(), lock.stat().st_mtime_ns) == before
        assert not (root / "target").exists()
        peer.finish()
    finally:
        if peer.process.poll() is None:
            peer.process.kill()
            peer.process.wait(timeout=10)
    print("locked workspace inputs passed")


if __name__ == "__main__":
    main()
