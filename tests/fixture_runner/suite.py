"""Run independent fixtures concurrently and retain deterministic report order."""

from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Event

from .execute import execute
from .model import Result
from .targets import applicable


def run_suite(fixtures, common, work, update, jobs, *, compile_only=False):
    results = [None] * len(fixtures)
    interrupted = Event()

    def record(index, result):
        results[index] = result
        print(f"{result.status}: {result.name} ({result.seconds:.2f}s)", flush=True)
        if result.message:
            print(result.message, flush=True)

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        pending = {}
        for index, fixture in enumerate(fixtures):
            if not applicable(fixture.data, common.get("target")):
                record(
                    index,
                    Result(
                        fixture.name, "inapplicable", 0, message="target is not declared applicable"
                    ),
                )
                continue
            if compile_only and fixture.data.get("requires_target_execution", True):
                record(
                    index,
                    Result(
                        fixture.name, "execution-skipped", 0, message="requires target execution"
                    ),
                )
                continue
            future = pool.submit(
                execute, fixture, common, work / "cases" / str(index), update, interrupted
            )
            pending[future] = index
        try:
            for future in as_completed(pending):
                record(pending[future], future.result())
        except KeyboardInterrupt:
            interrupted.set()
            for future in pending:
                future.cancel()
            for future, index in pending.items():
                if results[index] is None:
                    result = (
                        Result(fixtures[index].name, "interrupted", 0)
                        if future.cancelled()
                        else future.result()
                    )
                    record(index, result)
        finally:
            interrupted.set()
    return results
