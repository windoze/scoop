"""Fixture applicability uses the compilation target, independently of its host."""

from scripts.target_catalog import TARGET_FACTS

from .model import ConfigurationError


def validate_when(condition):
    if not isinstance(condition, dict):
        raise ConfigurationError("when: expected a table")
    for key, value in condition.items():
        if key not in ("os", "arch", "env"):
            raise ConfigurationError(f"when: unknown target field {key}")
        choices = [value] if isinstance(value, str) else value
        known = {facts[key] for facts in TARGET_FACTS.values()}
        if (
            not isinstance(choices, list)
            or not choices
            or any(not isinstance(choice, str) or choice not in known for choice in choices)
        ):
            raise ConfigurationError(f"when.{key}: expected registered target values")


def applicable(data, target):
    if data.get("targets") and target not in data["targets"]:
        return False
    for key, value in data.get("when", {}).items():
        choices = [value] if isinstance(value, str) else value
        if TARGET_FACTS[target][key] not in choices:
            return False
    return True
