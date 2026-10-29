"""Sample a separately built symbolized Scoop executable, outside timing runs."""

import subprocess
import sys
import time

from cases import CASES
from common import BUILD, LOGS, ROOT, environment, write_json


def main():
    binary = BUILD / "scoop-profile"
    symbols = next((BUILD / "profile/dumps").rglob("symbols.json"))
    report = []
    for case in (3, 7, 10, 12, 15, 16):
        name, n, _, _ = CASES[case]
        env = environment()
        env.update(
            {
                "BENCH_CASE": str(case),
                "BENCH_N": str(n),
                "BENCH_SEED": "42",
                "BENCH_WARMUP_MS": "500",
                "BENCH_SAMPLE_MS": "4000",
                "BENCH_SAMPLES": "1",
            }
        )
        output = LOGS / f"profile-{name}.stdout"
        error = LOGS / f"profile-{name}.stderr"
        raw = LOGS / f"profile-{name}.sample"
        named = LOGS / f"profile-{name}.named"
        with output.open("w") as stdout, error.open("w") as stderr:
            process = subprocess.Popen([binary], env=env, stdout=stdout, stderr=stderr)
            time.sleep(0.6)
            sample = subprocess.run(
                ["sample", str(process.pid), "3", "1", "-mayDie", "-file", str(raw)],
                capture_output=True,
                text=True,
            )
            status = process.wait()
        if sample.returncode or status:
            raise RuntimeError(f"profile failed: {name}: {sample.stderr}: {error.read_text()}")
        subprocess.run(
            [
                sys.executable,
                ROOT.parent / "tests/benchmarks/symbolize.py",
                "--symbols",
                symbols,
                "--input",
                raw,
                "--output",
                named,
                "--binary",
                binary,
                "--llvm-tools",
                "/opt/homebrew/opt/llvm@22/bin",
            ],
            check=True,
        )
        report.append(
            {
                "case": name,
                "executable": str(binary),
                "duration_seconds": 3,
                "interval_ms": 1,
                "stdout": output.read_text(),
                "sample": named.read_text(),
            }
        )
        write_json(ROOT / "results/2026-10-29/profiles.json", report)
        print(f"profiled {name}", flush=True)


if __name__ == "__main__":
    main()
