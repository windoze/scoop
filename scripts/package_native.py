"""Copy the host tools' actual shared dependencies and use relative loader paths."""

import os
import re
import shutil
import subprocess
from pathlib import Path


def output(*args):
    return subprocess.check_output(args, text=True, env=dict(os.environ, LC_ALL="C")).strip()


def darwin_rpaths(path):
    paths = []
    selected = False
    for line in output("otool", "-l", str(path)).splitlines():
        line = line.strip()
        if line.startswith("cmd "):
            selected = line == "cmd LC_RPATH"
        elif selected and line.startswith("path "):
            paths.append(line[5:].rsplit(" (offset ", 1)[0])
    return list(dict.fromkeys(paths))


def loader_path(name, source, executable):
    for prefix, root in (("@loader_path", source.parent), ("@executable_path", executable)):
        if name == prefix or name.startswith(prefix + "/"):
            return root / name[len(prefix) :].lstrip("/")
    return Path(name)


def darwin(root, binaries, llvm_lib):
    private = root / "libexec/llvm/lib"
    private.mkdir(parents=True)
    files = {path.resolve(): root / "bin" / path.name for path in binaries}
    executable = binaries[0].parent
    queue = [(path.resolve(), []) for path in binaries]
    libraries = {}
    edits = []
    while queue:
        source, inherited = queue.pop(0)
        destination = files[source]
        rpaths = darwin_rpaths(source)
        search = [loader_path(path, source, executable) for path in rpaths] + inherited
        arguments = []
        names = output("otool", "-D", str(source)).splitlines()[1:]
        own_name = names[0] if names else None
        for line in output("otool", "-L", str(source)).splitlines()[1:]:
            name = line.strip().rsplit(" (compatibility version ", 1)[0]
            if name == own_name or name.startswith(("/usr/lib/", "/System/Library/")):
                continue
            if name.startswith("@rpath/"):
                candidates = [path / name[7:] for path in [*search, llvm_lib]]
            else:
                candidates = [loader_path(name, source, executable)]
            dependency = next((path.resolve() for path in candidates if path.is_file()), None)
            if dependency is None:
                raise ValueError(f"cannot locate dependency {name} of {source}")
            previous = libraries.get(dependency.name)
            if previous is not None and previous != dependency:
                raise ValueError(
                    f"different host libraries have the same name: {previous}, {dependency}"
                )
            libraries[dependency.name] = dependency
            if dependency not in files:
                files[dependency] = private / dependency.name
                shutil.copy2(dependency, files[dependency])
                queue.append((dependency, search))
            arguments += ["-change", name, "@rpath/" + dependency.name]
        has_private_dependencies = bool(arguments)
        for path in rpaths:
            arguments += ["-delete_rpath", path]
        relative = (
            "@loader_path"
            if destination.parent == private
            else "@executable_path/../libexec/llvm/lib"
        )
        if has_private_dependencies:
            arguments += ["-add_rpath", relative]
        if own_name is not None:
            arguments += ["-id", "@rpath/" + destination.name]
        if arguments:
            edits.append((destination, arguments))
    for destination, arguments in reversed(edits):
        subprocess.run(["install_name_tool", *arguments, str(destination)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(destination)], check=True)
    return list(libraries.values())


def linux_system(name):
    return (
        name.startswith(("ld-linux-", "linux-vdso."))
        or re.fullmatch(r"lib(?:c|m|dl|pthread|rt|resolv|util|anl)\.so(?:\..+)?", name) is not None
    )


def linux(root, binaries, llvm_lib):
    if shutil.which("patchelf") is None:
        raise ValueError("Linux packaging requires patchelf")
    private = root / "libexec/llvm/lib"
    private.mkdir(parents=True)
    files = {path.resolve(): root / "bin" / path.name for path in binaries}
    queue = [path.resolve() for path in binaries]
    libraries = {}
    edits = []
    environment = dict(os.environ, LC_ALL="C", LD_LIBRARY_PATH=str(llvm_lib))
    while queue:
        source = queue.pop(0)
        needed = output("patchelf", "--print-needed", str(source)).splitlines()
        resolved = {}
        listing = subprocess.check_output(["ldd", str(source)], text=True, env=environment)
        for line in listing.splitlines():
            match = re.match(r"\s*(\S+) => (.*?) \(0x[0-9a-f]+\)", line)
            if match:
                resolved[match[1]] = Path(match[2]).resolve()
        arguments = []
        for name in needed:
            if linux_system(Path(name).name):
                continue
            dependency = Path(name).resolve() if Path(name).is_absolute() else resolved.get(name)
            if dependency is None or not dependency.is_file():
                raise ValueError(f"cannot locate dependency {name} of {source}")
            previous = libraries.get(dependency.name)
            if previous is not None and previous != dependency:
                raise ValueError(
                    f"different host libraries have the same name: {previous}, {dependency}"
                )
            libraries[dependency.name] = dependency
            if dependency not in files:
                files[dependency] = private / dependency.name
                shutil.copy2(dependency, files[dependency])
                queue.append(dependency)
            arguments += ["--replace-needed", name, dependency.name]
        destination = files[source]
        relative = "$ORIGIN" if destination.parent == private else "$ORIGIN/../libexec/llvm/lib"
        arguments += ["--set-rpath", relative]
        if destination.parent == private:
            arguments += ["--set-soname", destination.name]
        edits.append((destination, arguments))
    for destination, arguments in edits:
        subprocess.run(["patchelf", *arguments, str(destination)], check=True)
    return list(libraries.values())


def windows_system(name):
    name = name.lower()
    return name.startswith(("api-ms-win-", "ext-ms-win-")) or name in {
        "advapi32.dll",
        "bcrypt.dll",
        "bcryptprimitives.dll",
        "crypt32.dll",
        "cryptbase.dll",
        "dbghelp.dll",
        "gdi32.dll",
        "iphlpapi.dll",
        "kernel32.dll",
        "msvcrt.dll",
        "ntdll.dll",
        "ole32.dll",
        "oleaut32.dll",
        "psapi.dll",
        "rpcrt4.dll",
        "secur32.dll",
        "shell32.dll",
        "shlwapi.dll",
        "ucrtbase.dll",
        "user32.dll",
        "userenv.dll",
        "version.dll",
        "winhttp.dll",
        "winmm.dll",
        "ws2_32.dll",
    }


def windows(root, binaries, llvm_bin):
    """Put each executable's non-system PE dependencies beside that executable."""
    readobj = llvm_bin / "llvm-readobj.exe"
    queue = [(path.resolve(), root / "bin") for path in binaries]
    queue += [(path, path.parent) for path in (root / "commands").rglob("*.exe")]
    search = [Path(path) for path in os.get_exec_path() if path]
    libraries, copied, visited = {}, set(), set()
    while queue:
        source, directory = queue.pop(0)
        if (source, directory) in visited:
            continue
        visited.add((source, directory))
        imports = re.findall(
            r"^  Name: (.+)$", output(str(readobj), "--coff-imports", str(source)), re.M
        )
        for name in imports:
            if windows_system(name):
                continue
            dependency = next(
                (
                    path.resolve()
                    for parent in [llvm_bin, source.parent, *search]
                    if (path := parent / name).is_file()
                ),
                None,
            )
            if dependency is None:
                raise ValueError(f"cannot locate dependency {name} of {source}")
            key = name.lower()
            previous = libraries.get(key)
            if previous is not None and previous != dependency:
                raise ValueError(
                    f"different host libraries have the same name: {previous}, {dependency}"
                )
            libraries[key] = dependency
            destination = directory / dependency.name
            if (directory, key) not in copied:
                if dependency != destination.resolve():
                    shutil.copy2(dependency, destination)
                copied.add((directory, key))
                queue.append((dependency, directory))
    return list(libraries.values())
