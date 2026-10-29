"""Build all three source versions with the installed Scoop toolchain."""

import argparse
import shutil

from common import BUILD, JAVA_HOME, ROOT, SCOOP, environment, execute, version, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=lambda p: ROOT / p, default=ROOT / "results/2026-10-29/build.json"
    )
    parser.add_argument("--native", action="store_true", help="Also build GraalVM Native Image")
    args = parser.parse_args()
    BUILD.mkdir(exist_ok=True)
    report = {
        "versions": {
            "scoop": version([str(SCOOP), "--version"]),
            "go": version(["go", "version"]),
            "java": version([str(JAVA_HOME / "bin/java"), "-version"]),
        },
        "builds": [],
    }
    env = environment()
    commands = {
        "scoop": [
            SCOOP,
            "build",
            ROOT / "scoop",
            "--release",
            "--cache-dir",
            BUILD / "app-cache",
            "--target-dir",
            BUILD / "scoop-target",
            "-o",
            BUILD / "scoop-bench",
        ],
        "go": ["go", "build", "-o", BUILD / "go-bench", *sorted((ROOT / "go").glob("*.go"))],
        "javac": [
            JAVA_HOME / "bin/javac",
            "-Xlint:all",
            "-Werror",
            "-d",
            BUILD / "java",
            *sorted((ROOT / "java").glob("*.java")),
        ],
    }
    for name, command in commands.items():
        result = execute(command, f"build-{name}", env)
        report["builds"].append(
            {"name": name, "cache": "existing dependency caches, first suite build", **result}
        )
        print(f"{name}: {result['wall_seconds']:.3f}s", flush=True)
        write_json(args.output, report)
    if args.native:
        command = [
            JAVA_HOME / "bin/native-image",
            "--no-fallback",
            "-O2",
            "--parallelism=6",
            "-cp",
            BUILD / "java",
            "Bench",
            "-o",
            BUILD / "java-native",
        ]
        result = execute(command, "build-native", env)
        report["builds"].append({"name": "java-native", "cache": "full AOT image build", **result})
        print(f"java-native: {result['wall_seconds']:.3f}s", flush=True)
    for name in ("scoop", "go", "javac"):
        result = execute(commands[name], f"build-{name}-repeat", env)
        report["builds"].append({"name": name, "cache": "unchanged repeat", **result})
    report["sizes_bytes"] = {
        name: (BUILD / name).stat().st_size
        for name in ("scoop-bench", "go-bench", "java-native")
        if (BUILD / name).exists()
    }
    report["sizes_bytes"]["java-classfiles"] = sum(
        p.stat().st_size for p in (BUILD / "java").glob("*.class")
    )
    report["tools"] = {"go": shutil.which("go"), "scoop": str(SCOOP), "java_home": str(JAVA_HOME)}
    write_json(args.output, report)


if __name__ == "__main__":
    main()
