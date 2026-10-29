"""Prepare shared core source sets used by ordinary language fixtures."""

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
            (companions / "core-cast-direct.scoop").read_text(encoding="utf-8"),
        ),
        (
            "throwable.scoop",
            'public class ArithmeticException public constructor() : Exception(Some("arithmetic error"))\n',
            (companions / "core-arithmetic-direct.scoop").read_text(encoding="utf-8"),
        ),
    ]
    for name, old, new in replacements:
        path = sources / name
        text = path.read_text(encoding="utf-8")
        if text.count(old) != 1:
            raise ValueError(f"{path}: expected one class-fixture replacement: {old!r}")
        path.write_text(text.replace(old, new), encoding="utf-8", newline="\n")


def prepare_inheritance_core(repo, work, sysroot):
    destination = work / "inheritance-core"
    if destination.exists():
        return
    prepare_class_core(repo, work, sysroot)
    shutil.copytree(work / "class-core", destination, symlinks=True)
    source = destination / "src/user_class.scoop"
    parent = repo / "tests/fixtures/language/imported-classes/core-inheritance-provider.scoop"
    source.write_text(
        source.read_text(encoding="utf-8") + parent.read_text(encoding="utf-8"),
        encoding="utf-8",
        newline="\n",
    )


def prepare_member_defaults_core(repo, work, sysroot):
    destination = work / "member-defaults-core"
    if destination.exists():
        return
    prepare_class_core(repo, work, sysroot)
    shutil.copytree(work / "class-core", destination, symlinks=True)
    sources = destination / "src"
    source = sources / "user_class.scoop"
    defaults = repo / "tests/fixtures/language/imported-classes/core-member-defaults.scoop"
    source.write_text(
        source.read_text(encoding="utf-8") + defaults.read_text(encoding="utf-8"),
        encoding="utf-8",
        newline="\n",
    )
    declarations = [
        ("integer_long.scoop", "    Equals<Long>, ToString, Hash, RebuiltReadable {"),
        ("types.scoop", "public struct Boolean : Not<Boolean>, Equals<Boolean>, ToString, Hash {"),
        (
            "strings.scoop",
            "public class String : Compare<String>, Add<String, String>, Equals<String>, "
            "ToString, Hash, Iterable<Char> {",
        ),
    ]
    for name, old in declarations:
        path = sources / name
        text = path.read_text(encoding="utf-8")
        if text.count(old) != 1:
            raise ValueError(f"{path}: expected one member-defaults replacement: {old!r}")
        path.write_text(
            text.replace(old, old.removesuffix(" {") + ", RebuiltDefaults {"),
            encoding="utf-8",
            newline="\n",
        )
