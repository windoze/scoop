"""Prepare the two unchanged core source sets used by many language fixtures."""

import shutil


def prepare_test_core(repo, work, sysroot):
    destination = work / "test-core"
    if not destination.exists():
        shutil.copytree(sysroot / "src/scoop.core", destination, symlinks=True)
        shutil.copy2(
            repo / "tests/fixtures/language/imported-classes/support/stage3_test.scoop",
            destination / "src/stage3_test.scoop",
        )


def prepare_class_core(repo, work, sysroot):
    destination = work / "class-core"
    if destination.exists():
        return
    prepare_test_core(repo, work, sysroot)
    shutil.copytree(work / "test-core", destination, symlinks=True)
    sources = destination / "src"
    companions = repo / "tests/fixtures/language/imported-classes"
    shutil.copy2(companions / "core-provider.scoop", sources / "user_class.scoop")
    replacements = [
        (
            "integer_long.scoop",
            "    Equals<Long>, ToString, Hash {",
            "    Equals<Long>, ToString, Hash, RebuiltReadable {\n"
            "    public override fun read(): Long = this + stage3CoreAnswer() - 43",
        ),
        ("stage3_test.scoop", "= 42", "= 43"),
        (
            "throwable.scoop",
            'public class ClassCastException public constructor() : Exception(Some("invalid cast"))\n',
            (companions / "core-cast-direct.scoop").read_text(),
        ),
        (
            "throwable.scoop",
            'public class ArithmeticException public constructor() : Exception(Some("arithmetic error"))\n',
            (companions / "core-arithmetic-direct.scoop").read_text(),
        ),
    ]
    for name, old, new in replacements:
        path = sources / name
        text = path.read_text()
        if text.count(old) != 1:
            raise ValueError(f"{path}: expected one class-fixture replacement: {old!r}")
        path.write_text(text.replace(old, new))
