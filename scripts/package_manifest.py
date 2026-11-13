"""Produce and merge Scoop release manifests from successfully archived components."""

import argparse
import hashlib
import json
import tomllib
from pathlib import Path

from target_catalog import host_definition

IDENTITY = ("schema", "release", "version", "revision", "sources", "vendor")
SECTIONS = ("hosts", "targets", "archives")


def write(path, manifest):
    lines = []

    def table(values, prefix=()):
        if prefix:
            lines.extend(("", "[" + ".".join(json.dumps(key) for key in prefix) + "]"))
        for key, value in values.items():
            if not isinstance(value, dict):
                lines.append(f"{json.dumps(key)} = {json.dumps(value, ensure_ascii=False)}")
        for key, value in values.items():
            if isinstance(value, dict):
                table(value, (*prefix, key))

    table(manifest)
    path.parent.mkdir(parents=True, exist_ok=True)
    staged = path.with_name(path.name + ".new")
    try:
        staged.write_text("\n".join(lines) + "\n", encoding="utf-8")
        staged.replace(path)
    finally:
        staged.unlink(missing_ok=True)


def validate(manifest):
    if manifest["schema"] != 1:
        raise ValueError("Unsupported release manifest schema")
    references = {manifest["sources"], manifest["vendor"]}
    for host, selection in manifest["hosts"].items():
        if selection["default-target"] not in selection["targets"]:
            raise ValueError(f"Default target is not available for {host}")
        references.add(selection["bin"])
        for target in selection["targets"]:
            if target not in manifest["targets"]:
                raise ValueError(f"Missing target {target} for {host}")
    for selection in manifest["targets"].values():
        references.add(selection["base"])
        references.update(
            selection[key] for key in ("gui", "native", "profiling") if key in selection
        )
    missing = references - manifest["archives"].keys()
    if missing:
        raise ValueError(f"Missing component archives: {', '.join(sorted(missing))}")


def produce(output, version, revision, host, targets, packages, release):
    archives = {}
    prefix = f"scoop-{version}-"
    for name in packages:
        if not name.startswith(prefix) or not name.endswith(".tar.xz"):
            raise ValueError(f"Unexpected component filename: {name}")
        path = output / name
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        archives[name[len(prefix) : -len(".tar.xz")]] = {
            "path": name,
            "size": path.stat().st_size,
            "sha256": digest,
        }
    default = host_definition(host)["default_target"]
    manifest = {
        "schema": 1,
        "release": version,
        "version": version,
        "revision": revision,
        "sources": "sources",
        "vendor": "vendor",
        "hosts": {
            host: {
                "default-target": default,
                "targets": targets,
                "bin": "bin-" + host,
                "requirements": release["targets"][default],
            }
        },
        "targets": {},
        "archives": archives,
    }
    for target in targets:
        selection = {"base": "lib-base-" + target, "requirements": release["targets"][target]}
        for key, component in (
            ("gui", "lib-gui"),
            ("native", "native"),
            ("profiling", "lib-profiling"),
        ):
            if (name := component + "-" + target) in archives:
                selection[key] = name
        manifest["targets"][target] = selection
    validate(manifest)
    write(output / "manifest.toml", manifest)
    identity = {key: manifest[key] for key in IDENTITY}
    shared = {key: value for key, value in archives.items() if key in ("sources", "vendor")}
    write(output / "manifest-shared.toml", {**identity, "archives": shared})
    write(
        output / f"manifest-{host}.toml",
        {
            **identity,
            "hosts": manifest["hosts"],
            "targets": manifest["targets"],
            "archives": {key: value for key, value in archives.items() if key not in shared},
        },
    )
    return manifest


def merge(paths):
    combined = {key: {} for key in SECTIONS}
    for path in paths:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        for key in IDENTITY:
            if key in combined and combined[key] != manifest[key]:
                raise ValueError(f"Release identity differs in {path}: {key}")
            combined[key] = manifest[key]
        for section in SECTIONS:
            for key, value in manifest.get(section, {}).items():
                if key in combined[section] and combined[section][key] != value:
                    raise ValueError(f"Conflicting {section}.{key} in {path}")
                combined[section][key] = value
    validate(combined)
    return combined


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("fragments", type=Path, nargs="+")
    args = parser.parse_args()
    write(args.output, merge(args.fragments))


if __name__ == "__main__":
    main()
