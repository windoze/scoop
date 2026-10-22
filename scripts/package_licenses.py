"""Collect the licenses of the components in a manually assembled toolchain."""

import json
import shutil
import subprocess
from pathlib import Path


def license_files(directory):
    return sorted(
        path
        for path in directory.iterdir()
        if path.is_file()
        and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE"))
    )


def collect(root, repository, source, host, native):
    destination = root / "share/licenses"
    destination.mkdir(parents=True)
    rows = [
        "# Included component licenses",
        "",
        "| Component | Declared license | Documents |",
        "| --- | --- | --- |",
    ]

    def add(name, license_name, paths):
        folder = destination / name
        folder.mkdir()
        documents = []
        for path in paths:
            shutil.copy2(path, folder / path.name)
            documents.append(f"[{path.name}]({name}/{path.name})")
        rows.append(
            f"| {name} | {license_name or 'Not declared'} | {', '.join(documents) or 'No separate document found'} |"
        )

    own = license_files(repository)
    add("Scoop", "See project license" if own else "Not yet selected by the maintainer", own)
    add("LLVM", "Apache-2.0 WITH LLVM-exception", [source / "llvm/LICENSE.TXT"])
    for vendor in (repository / "sysroot/vendor").iterdir():
        if vendor.is_dir() and vendor.name != "llvm-project":
            add(vendor.name, "See included documents", license_files(vendor))
            if vendor.name == "text_backend":
                for package in json.loads((vendor / "packages.json").read_text()):
                    name = package["name"] + "-" + package["version"]
                    add(
                        "text-" + name,
                        package["license"],
                        license_files(vendor / "licenses" / name),
                    )

    metadata = json.loads(
        subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                host,
            ],
            cwd=repository,
        )
    )
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    queue = [
        package["id"]
        for package in packages.values()
        if package["name"] in {"scoop", "scoopc", "scoop-linker"}
    ]
    visited = set()
    while queue:
        identity = queue.pop()
        if identity in visited:
            continue
        visited.add(identity)
        package = packages[identity]
        if package["source"] is not None:
            directory = Path(package["manifest_path"]).parent
            paths = license_files(directory)
            if package["license_file"]:
                explicit = directory / package["license_file"]
                if explicit not in paths:
                    paths.append(explicit)
            add(f"{package['name']}-{package['version']}", package["license"], paths)
        queue.extend(
            dependency["pkg"]
            for dependency in nodes[identity]["deps"]
            if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])
        )

    if any(path.name.startswith(("libstdc++", "libgcc_s")) for path in native):
        paths = [
            path
            for path in (
                Path("/usr/share/doc/libstdc++6/copyright"),
                Path("/usr/share/doc/libgcc-s1/copyright"),
            )
            if path.is_file()
        ]
        # Debian copyright files cover the GCC runtime and its Runtime Library Exception.
        add("GCC-runtime", "GPL with GCC Runtime Library Exception", paths[:1])
    (destination / "README.md").write_text("\n".join(rows) + "\n")
