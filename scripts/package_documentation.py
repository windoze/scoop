"""Generate the target-specific built-in API sites included in a toolchain."""

import html
import shutil
import subprocess


def generate(sysroot, targets, binaries, work_dir, environment):
    roots = sorted((sysroot / "src").glob("*/Cone.toml"))
    if not roots:
        raise RuntimeError(f"no built-in Cone manifests found in {sysroot / 'src'}")
    sites = {}
    for target in targets:
        site = work_dir / "documentation" / target
        subprocess.run(
            [
                str(binaries[0]),
                "doc",
                *map(str, roots),
                "--target",
                target,
                "--sysroot",
                str(sysroot),
                "--scoopc",
                str(binaries[1]),
                "--no-deps",
                "--deny-doc-warnings",
                "--cache-dir",
                str(work_dir / "cache"),
                "--target-dir",
                str(work_dir / "doc-target"),
                "--output-dir",
                str(site),
            ],
            check=True,
            env=environment,
        )
        sites[target] = site
    return sites


def install(destination, sites):
    destination.mkdir(parents=True)
    links = []
    for target, source in sites.items():
        shutil.copytree(
            source, destination / target, ignore=shutil.ignore_patterns(".scoop-doc.json")
        )
        label = html.escape(target, quote=True)
        links.append(f'<li><a href="{label}/index.html">{label}</a></li>')
    (destination / "index.html").write_text(
        '<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        "<title>Scoop built-in API documentation</title></head>"
        "<body><main><h1>Scoop built-in API documentation</h1>"
        "<p>Select the target used to compile your application. "
        "All pages, source text, and search resources are included locally.</p><ul>"
        + "".join(links)
        + "</ul></main></body></html>\n",
        encoding="utf-8",
    )
