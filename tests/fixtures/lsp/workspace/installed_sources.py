"""Installed source navigation keeps the selected precompiled dependency semantics."""

import json
import shutil
import sys
from pathlib import Path


def location(sysroot, relative, declaration, name):
    path = (Path(sysroot) / "src" / relative).resolve()
    lines = path.read_text().splitlines()
    line = next(index for index, text in enumerate(lines) if declaration in text)
    start = lines[line].index(name)
    return {
        "uri": path.as_uri(),
        "range": {
            "start": {"line": line, "character": start},
            "end": {"line": line, "character": start + len(name)},
        },
    }


def boolean_location(sysroot):
    return location(sysroot, "scoop.core/src/types.scoop", "public struct Boolean", "Boolean")


def main():
    from peer import Peer

    server, package_manager, root, sysroot, target = sys.argv[1:]
    root = Path(root).resolve()
    sysroot = Path(sysroot).resolve()
    source = root / "src/navigation.scoop"
    text = source.read_text()
    manifest = root / "Cone.toml"
    original_manifest = manifest.read_text()
    peer = Peer(server, package_manager, sysroot, target, root.parent / "pm-cache")
    peer.resolve(root)
    options = {
        "target": target,
        "sysroot": str(sysroot),
        "packageManager": package_manager,
        "packageCacheDir": str(peer.cache),
    }

    def query(method, line, token):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": source.as_uri()},
                "position": {"line": line, "character": text.splitlines()[line].index(token)},
            },
        )

    def core_navigation(selected):
        assert query("definition", 2, "Boolean") == boolean_location(selected)
        assert query("typeDefinition", 2, "value") == boolean_location(selected)
        expected = location(
            selected, "scoop.core/src/types.scoop", "override fun toString()", "toString"
        )
        assert query("definition", 2, "toString") == expected

    def std_navigation(selected):
        duration = location(
            selected, "scoop.std/src/time/duration.scoop", "public class Duration", "Duration"
        )
        assert query("definition", 0, "Duration") == duration
        assert query("definition", 3, "Duration") == duration
        assert query("typeDefinition", 3, "value") == duration
        expected = location(
            selected, "scoop.std/src/time/duration.scoop", "override fun toString()", "toString"
        )
        assert query("definition", 3, "toString") == expected
        expected = location(
            selected, "scoop.std/src/time/duration.scoop", "public val floorSeconds", "floorSeconds"
        )
        assert query("definition", 4, "floorSeconds") == expected

    def change_dependency(path, version):
        updated = original_manifest.replace(
            '"scoop:scoop.std" = "0.1.0"',
            f'"scoop:scoop.std" = {{version = "=0.1.0", artifact = {json.dumps(str(path))}}}',
        )
        manifest.write_text(updated)
        peer.resolve(root)
        if version == 1:
            peer.notify(
                "textDocument/didOpen",
                {
                    "textDocument": {
                        "uri": manifest.as_uri(),
                        "languageId": "toml",
                        "version": version,
                        "text": updated,
                    }
                },
            )
        else:
            peer.notify(
                "textDocument/didChange",
                {
                    "textDocument": {"uri": manifest.as_uri(), "version": version},
                    "contentChanges": [{"text": updated}],
                },
            )

    try:
        peer.request(
            "initialize",
            {
                "processId": None,
                "workspaceFolders": [{"name": "installed", "uri": root.as_uri()}],
                "initializationOptions": options,
                "capabilities": {},
            },
        )
        peer.notify("initialized", {})
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": source.as_uri(),
                    "languageId": "scoop",
                    "version": 1,
                    "text": text,
                }
            },
        )
        core_navigation(sysroot)
        std_navigation(sysroot)
        signature = query("hover", 3, "toString")
        assert signature["contents"]["value"] == "fun toString(): String", signature
        symbols = peer.request("workspace/symbol", {"query": "Duration"})
        assert {item["name"] for item in symbols} == {"durationText", "durationSeconds"}, symbols
        assert all(item["location"]["uri"] == source.as_uri() for item in symbols), symbols

        # An explicit locator into this sysroot uses the same source attachment.
        change_dependency(sysroot / "lib" / target / "scoop.std.slib", 1)
        std_navigation(sysroot)

        # A separate artifact stays usable without borrowing installed sources.
        external = root / "external.slib"
        shutil.copyfile(sysroot / "lib" / target / "scoop.std.slib", external)
        change_dependency(external, 2)
        assert query("definition", 0, "Duration") is None
        assert query("definition", 3, "toString") is None
        assert query("typeDefinition", 3, "value") is None
        assert query("hover", 3, "toString") == signature
        core_navigation(sysroot)
        manifest.write_text(original_manifest)
        peer.resolve(root)
        peer.notify("textDocument/didClose", {"textDocument": {"uri": manifest.as_uri()}})
        std_navigation(sysroot)

        # Keep original inputs and share artifacts while relocating the source package.
        relocated = root / "relocated"
        libraries = relocated / "lib" / target
        libraries.mkdir(parents=True)
        for name in ("scoop.core", "scoop.std"):
            libraries.joinpath(f"{name}.slib").symlink_to(sysroot / "lib" / target / f"{name}.slib")
        peer.notify(
            "workspace/didChangeConfiguration",
            {"settings": {"scoop": {**options, "sysroot": str(relocated)}}},
        )
        assert query("hover", 3, "toString") == signature
        assert query("definition", 2, "Boolean") is None
        assert query("definition", 0, "Duration") is None
        for name in ("scoop.core", "scoop.std"):
            shutil.copytree(
                sysroot / "src" / name,
                relocated / "src" / name,
                ignore=shutil.ignore_patterns("tests", "consumer-tests", "examples", "target"),
            )
        peer.notify(
            "workspace/didChangeConfiguration",
            {"settings": {"scoop": {**options, "sysroot": str(relocated)}}},
        )
        core_navigation(relocated)
        std_navigation(relocated)

        # Text used for navigation is not compiled as a source dependency.
        attached = relocated / "src/scoop.std/src/time/duration.scoop"
        with attached.open("a") as stream:
            stream.write("\nfun unfinished(\n")
        peer.notify(
            "workspace/didChangeWatchedFiles",
            {"changes": [{"uri": attached.as_uri(), "type": 2}]},
        )
        assert query("hover", 3, "toString") == signature
        std_navigation(relocated)
        peer.notify("workspace/didChangeConfiguration", {"settings": {"scoop": options}})
        core_navigation(sysroot)
        std_navigation(sysroot)
        assert all(not item["diagnostics"] for item in peer.diagnostics.values()), peer.diagnostics
        assert not (root / "target").exists()
        peer.finish()
    finally:
        if peer.process.poll() is None:
            peer.process.kill()
            peer.process.wait(timeout=10)
    print("installed source navigation passed")


if __name__ == "__main__":
    main()
