"""Defaults, nested captures, overloads and failed bodies obey rename boundaries."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-bodies"
    manifest(project, "rename-bodies")
    file = "src/bodies.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            file: (
                "package renamebodies\n"
                "fun choose(value: Boolean): Boolean = value\n"
                "fun choose(value: Boolean, second: Boolean): Boolean = second\n"
                "fun use(): Boolean = choose(true)\n"
                "fun defaults(first: Boolean, second: Boolean = first): Boolean = second\n"
                "fun named(): Boolean = defaults(first = true)\n"
                "fun capture(input: Boolean): Boolean {\n"
                "    val captured = input\n"
                "    fun nested(): Boolean = captured\n"
                "    val lambda: () -> Boolean = { captured }\n"
                "    return nested()\n"
                "}\n"
            )
        },
    )
    case.clean()
    case.rename(file, "choose(true)", "choose", "select", 2)
    assert "fun choose(value: Boolean, second" in case.texts[file], case.texts[file]
    case.rename(file, "first: Boolean", "first", "initial", 3)
    case.rename(file, "val captured", "captured", "saved", 3)
    case.rename(file, "fun nested", "nested", "inner", 2)
    saved = case.texts[file]
    case.change(file, saved + "fun broken(): Boolean = unknown\n")
    case.query(file, "fun select", "select", "prepareRename", error=-32803)
    case.query(file, "fun select", "select", "rename", error=-32803, newName="renamed")
    case.change(file, saved)
    case.clean()
    case.rename(file, "fun select", "select", "renamed", 2)
    case.change(
        file,
        case.texts[file]
        + "class Operator { operator fun plus(other: Boolean): Boolean = other }\n"
        + "fun operate(value: Operator): Boolean = value + true\n",
    )
    case.clean()
    assert case.query(file, "value + true", "+", "prepareRename") is None
    failure = case.query(file, "fun plus", "plus", "prepareRename", error=-32803)
    assert "operator" in failure["message"], failure
    case.query(file, "fun plus", "plus", "rename", error=-32803, newName="add")
    case.finish()
