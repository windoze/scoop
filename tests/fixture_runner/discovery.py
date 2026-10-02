"""Automatic discovery, carrier validation, and explicit source ownership."""

import tomllib
from pathlib import Path

from .model import ConfigurationError, Fixture
from .schema import validate


def input_files(base, names):
    result = set()
    for name in names:
        target = base / name
        if not target.exists():
            raise ConfigurationError(f"missing declared input: {target}")
        if target.is_dir():
            result.update(item.resolve() for item in target.rglob("*") if item.is_file())
        else:
            result.add(target.resolve())
    return frozenset(result)


def inline(source):
    lines = source.read_bytes().decode("utf-8").splitlines()
    if not lines or lines[0] != "// fixture:begin":
        return None
    result = []
    for line in lines[1:]:
        if line == "// fixture:end":
            return tomllib.loads("\n".join(result))
        if not line.startswith("//"):
            raise ConfigurationError(f"{source}: malformed inline fixture")
        result.append(line[2:].removeprefix(" "))
    raise ConfigurationError(f"{source}: unterminated inline fixture")


def expectation_files(value, base, update):
    if isinstance(value, list):
        for item in value:
            expectation_files(item, base, update)
    elif isinstance(value, dict):
        for key, item in value.items():
            if key in ("file", "snapshot") and isinstance(item, str) and "${" not in item:
                if not (base / item).is_file() and not (update and key == "snapshot"):
                    raise ConfigurationError(f"missing expectation file: {base / item}")
            else:
                expectation_files(item, base, update)


def discover(suite: Path, update=False):
    sources = set(suite.rglob("*.scoop"))
    carriers = sorted(set(suite.rglob("*.fixture.toml")) | set(suite.rglob("fixture.toml")))
    raw = [(carrier, tomllib.loads(carrier.read_text())) for carrier in carriers]
    for source in sorted(sources):
        data = inline(source)
        if data is not None:
            if source.with_suffix(".fixture.toml").exists():
                raise ConfigurationError(f"{source}: duplicate inline and sidecar criteria")
            raw.append((source, data))
    fixtures, names, covered = [], set(), set()
    for locator, data in raw:
        try:
            validate(data)
            base = locator.parent
            expectation_files(data["steps"], base, update)
            inputs = input_files(base, data["inputs"])
            support = input_files(base, data.get("support", []))
            if data["name"] in names:
                raise ConfigurationError(f"duplicate fixture name {data['name']}")
            if "root" in data and not (base / data["root"]).exists():
                raise ConfigurationError(f"missing root {data['root']}")
            if locator.suffix == ".scoop" and locator.resolve() not in inputs:
                raise ConfigurationError("inline fixture must own its source")
            fixtures.append(Fixture(locator, base, data["name"], data, inputs, support))
            names.add(data["name"])
            covered.update(inputs | support)
        except (ValueError, TypeError, KeyError) as error:
            raise ConfigurationError(f"{locator}: {error}") from error
    uncovered = {source.resolve() for source in sources} - covered
    if uncovered:
        examples = "\n".join(str(item) for item in sorted(uncovered)[:30])
        raise ConfigurationError(
            f"{len(uncovered)} source files have no declared ownership:\n{examples}"
        )
    return sorted(fixtures, key=lambda fixture: fixture.name)
