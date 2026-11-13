"""Collect the licenses of the components in a manually assembled toolchain."""

import json
import shutil
import subprocess
from pathlib import Path

from build_rust_text import NOTICE_DIRECTORY
from build_skia import TARGETS as SKIA_TARGETS
from prepare_gtk import NOTICE_DIRECTORY as GTK_NOTICE_DIRECTORY
from prepare_gtk import TARGET as GTK_TARGET
from prepare_tray import NOTICE_DIRECTORY as TRAY_NOTICE_DIRECTORY
from prepare_tray import TARGET as TRAY_TARGET
from target_catalog import host_definition, llvm_mingw_resource_triple


def llvm_mingw_license_root(prefix, architecture):
    return prefix / llvm_mingw_resource_triple(architecture) / "share/mingw32"


def license_files(directory):
    return sorted(
        path
        for path in directory.iterdir()
        if path.is_file()
        and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE"))
    )


class Collector:
    """Write one component's license table and documents under share/licenses."""

    def __init__(self, root, component=None):
        self.destination = root / "share/licenses"
        if component is not None:
            self.destination /= component
        self.destination.mkdir(parents=True, exist_ok=True)
        self.rows = [
            "# Included component licenses",
            "",
            "| Component | Declared license | Documents |",
            "| --- | --- | --- |",
        ]

    def add(self, name, license_name, paths):
        folder = self.destination / name
        folder.mkdir()
        documents = []
        for path in paths:
            shutil.copy2(path, folder / path.name)
            documents.append(f"[{path.name}]({name}/{path.name})")
        self.rows.append(
            f"| {name} | {license_name or 'Not declared'} | {', '.join(documents) or 'No separate document found'} |"
        )

    def finish(self):
        (self.destination / "README.md").write_text("\n".join(self.rows) + "\n")


def collect_bin(root, repository, host):
    """Licenses of Scoop itself and the Rust dependencies of the host commands."""
    collector = Collector(root)
    own = license_files(repository)
    collector.add(
        "Scoop", "See project license" if own else "Not yet selected by the maintainer", own
    )
    packages = {}
    dependencies = {}
    for workspace in (
        repository,
        repository / "tooling/scoop-example",
        repository / "tooling/scoop-bundle",
    ):
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
                cwd=workspace,
            )
        )
        packages.update((package["id"], package) for package in metadata["packages"])
        for node in metadata["resolve"]["nodes"]:
            # Shared crates may enable different features in the Cargo projects.
            dependencies.setdefault(node["id"], []).extend(node["deps"])
    queue = [
        package["id"]
        for package in packages.values()
        if package["name"]
        in {
            "scoop",
            "scoopc",
            "scoop-linker",
            "scoop-lsp",
            "scoop-pm",
            "scoop-pgo",
            "scoop-example",
            "scoop-bundle",
        }
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
            collector.add(f"{package['name']}-{package['version']}", package["license"], paths)
        queue.extend(
            dependency["pkg"]
            for dependency in dependencies[identity]
            if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])
        )
    collector.finish()


def collect_profiling(root, source, target):
    """The optional target component contains LLVM's profile runtime."""
    collector = Collector(root, "profiling-" + target)
    collector.add(
        "compiler-rt", "Apache-2.0 WITH LLVM-exception", [source / "compiler-rt/LICENSE.TXT"]
    )
    collector.finish()


def collect_llvm(root, source, host, native, python_license=None):
    """Licenses of the LLVM/LLDB toolchain package, including its Python runtime."""
    collector = Collector(root, "llvm")
    collector.add("LLVM", "Apache-2.0 WITH LLVM-exception", [source / "llvm/LICENSE.TXT"])
    if host_definition(host)["system"] == "Windows":
        for component in ("libcxx", "libcxxabi", "libunwind", "compiler-rt"):
            collector.add(component, "See included documents", [source / component / "LICENSE.TXT"])
    elif (root / "libexec/scoop/include/unwind").is_dir():
        collector.add(
            "libunwind", "Apache-2.0 WITH LLVM-exception", [source / "libunwind/LICENSE.TXT"]
        )
    if python_license is not None:
        collector.add("Python", "PSF-2.0 and included notices", [python_license])
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
        collector.add("GCC-runtime", "GPL with GCC Runtime Library Exception", paths[:1])
    collector.finish()


def collect_upstream(collector, sysroot, *, include_gui):
    """Include notices with both upstream sources and the binaries built from them."""
    for vendor in (sysroot / "vendor").iterdir():
        if vendor.is_dir() and vendor.name != "llvm-project":
            if vendor.name == "skia" and not include_gui:
                continue
            license_root = vendor / "2.15" if vendor.name == "liburing" else vendor
            collector.add(vendor.name, "See included documents", license_files(license_root))
            if vendor.name == "text_backend":
                for package in json.loads((vendor / "packages.json").read_text()):
                    name = package["name"] + "-" + package["version"]
                    collector.add(
                        "text-" + name,
                        package["license"],
                        license_files(vendor / "licenses" / name),
                    )


def collect_vendor(root, sysroot):
    collector = Collector(root, "vendor")
    collect_upstream(collector, sysroot, include_gui=True)
    collector.finish()


def collect_base_libraries(root, sysroot, target):
    """Licenses of one target's base package: Rust-built and toolchain components."""
    collector = Collector(root, "lib-base-" + target)
    notices = sysroot / "lib" / target / NOTICE_DIRECTORY
    collector.add("Rust-std-" + target, "See included documents", sorted(notices.iterdir()))
    collect_upstream(collector, sysroot, include_gui=False)
    collector.finish()


def collect_mingw(root, prefix, component, architecture):
    collector = Collector(root, component)
    collector.add(
        "LLVM-MinGW",
        "See included documents",
        [
            prefix / "LICENSE.TXT",
            *license_files(llvm_mingw_license_root(prefix, architecture)),
        ],
    )
    collector.finish()


def collect_gui_libraries(root, sysroot, target):
    """Licenses of one target's GUI package native components."""
    collector = Collector(root, "lib-gui-" + target)
    if target in SKIA_TARGETS:
        collector.add("Skia", "See included documents", license_files(sysroot / "vendor/skia"))
        collector.add(
            "Lucide",
            "ISC and included MIT notices",
            license_files(sysroot / "src/scoop.icons.lucide"),
        )
    if target == GTK_TARGET:
        for directory in sorted((sysroot / "lib" / target / GTK_NOTICE_DIRECTORY).iterdir()):
            collector.add(
                "GTK-" + directory.name, "See included documents", license_files(directory)
            )
    if target == TRAY_TARGET:
        for directory in sorted((sysroot / "lib" / target / TRAY_NOTICE_DIRECTORY).iterdir()):
            collector.add(
                "Tray-" + directory.name, "See included documents", license_files(directory)
            )
    collector.finish()
