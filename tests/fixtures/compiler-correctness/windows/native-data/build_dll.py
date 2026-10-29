"""Build the native provider DLL with the selected target compiler."""

import subprocess
import sys
from pathlib import Path


def main():
    compiler, target, output, source, *arguments = sys.argv[1:]
    output, source = Path(output), Path(source)
    library = output / "scoop_data.lib"
    import_option = (
        f"-Wl,/implib:{library}"
        if target == "x86_64-pc-windows-msvc-ucrt"
        else f"-Wl,--out-implib,{library}"
    )
    subprocess.run(
        [
            compiler,
            *arguments,
            "-DSCOOP_DATA_DLL=1",
            "-shared",
            "-fuse-ld=lld",
            "-Wall",
            "-Wextra",
            "-Werror",
            str(source / "provider.c"),
            str(source / "constant.c"),
            "-o",
            str(output / "scoop_data.dll"),
            import_option,
        ],
        check=True,
    )


if __name__ == "__main__":
    main()
