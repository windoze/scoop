"""Measure fresh application builds while keeping installed standard libraries available."""

import argparse
import shutil
from pathlib import Path

from common import BUILD, JAVA_HOME, ROOT, SCOOP, environment, execute, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "results/2026-10-29/build-cost.json")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--native-runs", type=int, default=3)
    parser.add_argument("--work", type=Path, default=BUILD / "build-cost")
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(exist_ok=False)
    base_cache = work / "go-dependency-cache"
    dependency_source = work / "dependencies.go"
    dependency_source.write_text(
        'package main\nimport (_ "fmt"; _ "os"; _ "strconv"; _ "time"; _ "strings"; _ "sync")\nfunc main() {}\n'
    )
    env = environment()
    dependency_env = dict(env, GOCACHE=str(base_cache))
    execute(
        ["go", "build", "-o", work / "dependencies", dependency_source],
        "go-prepare-dependencies",
        dependency_env,
    )
    report = {
        "definition": "Fresh application outputs/cache; installed/prebuilt standard libraries and OS cache remain warm. Cache copying and dependency preparation excluded.",
        "runs": [],
    }
    for trial in range(args.runs):
        trial_dir = work / str(trial)
        trial_dir.mkdir()
        go_cache = trial_dir / "go-cache"
        shutil.copytree(base_cache, go_cache)
        commands = {
            "scoop": [
                SCOOP,
                "build",
                ROOT / "scoop",
                "--release",
                "--cache-dir",
                trial_dir / "scoop-cache",
                "--target-dir",
                trial_dir / "scoop-target",
                "-o",
                trial_dir / "scoop",
            ],
            "go": ["go", "build", "-o", trial_dir / "go", *sorted((ROOT / "go").glob("*.go"))],
            "javac": [
                JAVA_HOME / "bin/javac",
                "-Xlint:all",
                "-Werror",
                "-d",
                trial_dir / "java",
                *sorted((ROOT / "java").glob("*.java")),
            ],
        }
        for language, command in commands.items():
            child_env = dict(env, GOCACHE=str(go_cache)) if language == "go" else env
            for phase in ("fresh-application", "unchanged-repeat"):
                result = execute(command, f"cost-{trial}-{language}-{phase}", child_env)
                report["runs"].append(
                    {"language": language, "phase": phase, "trial": trial, **result}
                )
                write_json(args.output, report)
                print(f"{trial} {language} {phase}: {result['wall_seconds']:.3f}s", flush=True)
    for trial in range(args.native_runs):
        result = execute(
            [
                JAVA_HOME / "bin/native-image",
                "--no-fallback",
                "-O2",
                "--parallelism=6",
                "-cp",
                BUILD / "java",
                "Bench",
                "-o",
                work / f"native-{trial}",
            ],
            f"cost-native-{trial}",
            env,
        )
        report["runs"].append(
            {
                "language": "java-native",
                "phase": "full-image-from-bytecode",
                "trial": trial,
                **result,
            }
        )
        write_json(args.output, report)
        print(f"{trial} java-native: {result['wall_seconds']:.3f}s", flush=True)


if __name__ == "__main__":
    main()
