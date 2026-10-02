"""One ordered executor for every fixture carrier, tool, and variant."""

import os
import time

from .assertions import check_all, process_expectations
from .files import prepare
from .model import (
    AssertionFailure,
    ConfigurationError,
    EnvironmentError,
    Interrupted,
    Result,
    check_interrupted,
)
from .process import Process
from .values import expand, path


def execute(fixture, common, work, update=False, interrupted=None):
    started = time.monotonic()
    result = Result(fixture.name, "passed", 0)
    variants = {}
    try:
        for variant in fixture.data.get("variants", [{"name": "normal"}]):
            check_interrupted(interrupted)
            directory = work / variant["name"]
            directory.mkdir(parents=True, exist_ok=True)
            context = common | {
                "fixture": str(fixture.base),
                "work": str(directory),
                "root": str(fixture.base / fixture.data.get("root", ".")),
                "variant": variant["name"],
                "variants": variants,
            }
            context |= expand(fixture.data.get("vars", {}), context)
            context |= expand(variant.get("vars", {}), context)
            environment = dict(os.environ)
            environment.pop("SCOOP_GC_STRESS_MOVE", None)
            environment |= expand(variant.get("env", {}), context)
            execute_variant(fixture, context, environment, update, result, interrupted)
            variants[variant["name"]] = context
            result.variants += 1
    except Interrupted as error:
        result.status, result.message = "interrupted", str(error)
    except ConfigurationError as error:
        result.status, result.message = "configuration-error", str(error)
    except EnvironmentError as error:
        result.status, result.message = "environment-error", str(error)
    except (AssertionFailure, OSError, ValueError) as error:
        result.status, result.message = "failed", str(error)
    result.seconds = time.monotonic() - started
    return result


def execute_variant(fixture, context, environment, update, result, interrupted):
    running = {}
    try:
        for step in fixture.data["steps"]:
            check_interrupted(interrupted)
            name = step["name"]
            context[name] = {}
            if "argv" in step:
                process = Process(step, context, fixture.base, environment)
                running[name] = process
                result.processes += 1
                context[name] = {"pid": process.child.pid}
                if not step.get("background"):
                    finish(name, process, fixture, context, update, result, interrupted)
                    del running[name]
                    continue
            elif "files" in step:
                prepare(step["files"], context, fixture.base)
            elif "wait" in step:
                process_name = step["wait"]
                finish(
                    process_name,
                    running[process_name],
                    fixture,
                    context,
                    update,
                    result,
                    interrupted,
                )
                del running[process_name]
            elif "send_signal" in step:
                value = step["send_signal"]
                running[value["process"]].send_signal(value["signal"])
            elif "await_file" in step:
                target = path(expand(step["await_file"], context), fixture.base)
                deadline = time.monotonic() + step.get("timeout", 30)
                while not target.exists():
                    check_interrupted(interrupted)
                    if time.monotonic() >= deadline:
                        raise AssertionFailure(f"{name}: timed out awaiting {target}")
                    time.sleep(0.02)
                context[name] = {"path": str(target), "bytes": target.read_bytes()}
            result.snapshots += check_all(step.get("checks", []), context, fixture.base, update)
    finally:
        for process in running.values():
            process.cleanup()


def finish(name, process, fixture, context, update, result, interrupted):
    context[name] = process.finish(interrupted)
    process_expectations(process.step, context[name], context, fixture.base)
    result.snapshots += check_all(process.step.get("checks", []), context, fixture.base, update)
