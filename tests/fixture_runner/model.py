"""Fixture data and errors shared by discovery and execution."""

from dataclasses import dataclass
from pathlib import Path
from typing import Any


class ConfigurationError(ValueError):
    pass


class EnvironmentError(RuntimeError):
    pass


class AssertionFailure(AssertionError):
    pass


class Interrupted(RuntimeError):
    pass


def check_interrupted(event):
    if event is not None and event.is_set():
        raise Interrupted("fixture run interrupted")


@dataclass(frozen=True)
class Fixture:
    locator: Path
    base: Path
    name: str
    data: dict[str, Any]
    inputs: frozenset[Path]
    support: frozenset[Path]


@dataclass
class Result:
    name: str
    status: str
    seconds: float
    variants: int = 0
    processes: int = 0
    snapshots: int = 0
    message: str = ""
