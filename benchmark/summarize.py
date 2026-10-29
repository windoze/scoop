"""Render measured results without combining unrelated workloads into a score."""

import argparse
import json
import statistics
from pathlib import Path

LANGUAGES = ("scoop", "go", "java-jit", "java-native")
LABELS = ("Scoop", "Go", "Graal JIT", "Native Image")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    directory = args.directory
    data = json.loads((directory / "measurements.json").read_text())
    lines = [
        "## 稳态时间",
        "",
        "单位：ms / 完整 kernel 调用，越小越好。括号为 3 个独立进程中位数的范围。",
        "",
        "| 工作负载 | Scoop | Go | Graal JIT | Native Image |",
        "|---|---:|---:|---:|---:|",
    ]
    for case in data["summary"]:
        cells = [case["case"]]
        for language in LANGUAGES:
            value = case["languages"][language]
            cells.append(
                f"{value['median_ms']:.3f} ({value['min_fork_ms']:.3f}–{value['max_fork_ms']:.3f})"
            )
        lines.append("| " + " | ".join(cells) + " |")
    lines += [
        "",
        "## 进程峰值 RSS",
        "",
        "单位：MiB，含预热，报告各进程峰值的中位数。",
        "",
        "| 工作负载 | Scoop | Go | Graal JIT | Native Image |",
        "|---|---:|---:|---:|---:|",
    ]
    for case in data["summary"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    case["case"],
                    *[f"{case['languages'][lang]['peak_rss_mib']:.1f}" for lang in LANGUAGES],
                ]
            )
            + " |"
        )
    startup = json.loads((directory / "startup.json").read_text())["summary"]
    lines += [
        "",
        "## 空工作进程启动／退出",
        "",
        "| 指标 | Scoop | Go | Graal JIT | Native Image |",
        "|---|---:|---:|---:|---:|",
    ]
    for key, title in [("median_ms", "中位时间 ms"), ("peak_rss_mib", "峰值 RSS MiB")]:
        lines.append(
            "| " + " | ".join([title, *[f"{startup[lang][key]:.3f}" for lang in LANGUAGES]]) + " |"
        )
    cost = json.loads((directory / "build-cost.json").read_text())
    lines += [
        "",
        "## 构建成本",
        "",
        "| 模式 | 应用新构建 s | 不改输入再次构建 s |",
        "|---|---:|---:|",
    ]
    for language in ("scoop", "go", "javac", "java-native"):
        values = []
        for phase in (
            "fresh-application" if language != "java-native" else "full-image-from-bytecode",
            "unchanged-repeat",
        ):
            samples = [
                r["wall_seconds"]
                for r in cost["runs"]
                if r["language"] == language and r["phase"] == phase
            ]
            values.append(f"{statistics.median(samples):.3f}" if samples else "无增量模式")
        lines.append("| " + " | ".join([language, *values]) + " |")
    (directory / "TABLES.md").write_text("\n".join(lines) + "\n")
    plot(data, directory)


def plot(data, directory):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import numpy as np

    rows = data["summary"]
    y = np.arange(len(rows))
    fig, ax = plt.subplots(figsize=(11.5, 10.5))
    colors = {"scoop": "#087f8c", "java-jit": "#d77a22", "java-native": "#7557ad"}
    for language, offset, label in [
        ("scoop", -0.21, "Scoop"),
        ("java-jit", 0, "Graal JIT"),
        ("java-native", 0.21, "Native Image"),
    ]:
        ratios, low, high = [], [], []
        for row in rows:
            v = row["languages"][language]
            base = row["languages"]["go"]["median_ms"]
            ratios.append(v["median_ms"] / base)
            low.append((v["median_ms"] - v["min_fork_ms"]) / base)
            high.append((v["max_fork_ms"] - v["median_ms"]) / base)
        ax.errorbar(
            ratios,
            y + offset,
            xerr=[low, high],
            fmt="o",
            markersize=5,
            capsize=2,
            color=colors[language],
            label=label,
        )
    ax.axvline(1, color="#444444", linestyle="--", linewidth=1)
    ax.set_yticks(y, [r["case"] for r in rows])
    ax.invert_yaxis()
    ax.set_xscale("log", base=2)
    ax.set_xticks([0.25, 0.5, 1, 2, 4, 8], ["0.25", "0.5", "1", "2", "4", "8"])
    ax.set_xlim(0.28, 9)
    ax.set_xlabel("Time / Go time (lower is faster; Go = 1)", labelpad=12)
    ax.set_title(
        "Scoop / Go / GraalVM 21 — Apple M4 Pro\n3 forks × 5 batches; bars show fork range, not confidence intervals",
        loc="left",
        pad=18,
    )
    ax.grid(axis="x", alpha=0.22)
    ax.legend(loc="lower right", framealpha=0.95)
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    fig.tight_layout()
    fig.savefig(directory / "relative-time.png", dpi=180)
    fig.savefig(directory / "relative-time.svg")
    plt.close(fig)


if __name__ == "__main__":
    main()
