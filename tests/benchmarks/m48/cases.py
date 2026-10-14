"""Real source inputs for cold, warm and native-header build measurements."""

import json
import shutil
from pathlib import Path


def prepare(repo: Path, work: Path, case: str):
    if case in ("core", "std"):
        return repo / "sysroot/lib" / f"scoop.{case}", None, None
    if case == "empty":
        source = work / "empty.scoop"
        source.write_text("fun main() {}\n")
        return source, "", None
    if case == "application":
        destination = work / "application"
        source = repo / "tests/fixtures/m42-application"
        for name in ("provider", "consumer", "application"):
            shutil.copytree(source / name, destination / name)
        for name in ("consumer", "application"):
            manifest = destination / name / "Cone.toml"
            text = manifest.read_text()
            for dependency in ("provider", "consumer"):
                key = f'"dev.m42:app-{dependency}"'
                locator = json.dumps(str(destination / dependency))
                text = text.replace(
                    f'{key} = "0.1.0"',
                    f'{key} = {{version = "0.1.0", path = {locator}}}',
                )
            manifest.write_text(text)
        return destination / "application", "std application passed\n", None
    if case == "native":
        destination = work / "native"
        shutil.copytree(repo / "tests/fixtures/m33-native-c/values/cone", destination)
        return destination, "native-c-ok\n", destination / "include/nested/value.h"
    raise ValueError(case)
