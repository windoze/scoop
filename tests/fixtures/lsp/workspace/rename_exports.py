"""Rename follows public export paths while preserving explicit alias names."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-exports"
    for directory, name, dependencies in [
        ("origin", "rename-origin", []),
        ("facade", "rename-facade", [("rename-origin", "../origin")]),
        ("relay", "rename-relay", [("rename-facade", "../facade")]),
        ("", "rename-consumer", [("rename-facade", "facade"), ("rename-relay", "relay")]),
    ]:
        manifest(project / directory, name, dependencies)
    original = "origin/src/api.scoop"
    facade = "facade/src/api.scoop"
    main = "src/main.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            original: (
                "package renameorigin\n"
                "public fun pick(value: Boolean): Boolean = value\n"
                "public fun other(value: Boolean): Boolean = value\n"
                "public class Box public constructor()\n"
            ),
            facade: (
                "package renamefacade\n"
                "public import renameorigin.pick as visible\n"
                "public import renameorigin.pick as alternate\n"
                "public import renameorigin.pick\n"
                "public import renameorigin.Box as VisibleBox\n"
                "fun facadeUse(): Boolean = visible(true)\n"
            ),
            "relay/src/api.scoop": "package renamerelay\npublic import renamefacade.*\n",
            main: (
                "import renamerelay.visible as localChoice\n"
                "import renamerelay.alternate\n"
                "import renamerelay.pick\n"
                "import renamerelay.VisibleBox as LocalBox\n"
                "fun use(value: LocalBox): Boolean {\n"
                "    val first = /* 🌟 */ localChoice(true)\n"
                "    val factory: (Boolean) -> Boolean = ::localChoice\n"
                "    val box = LocalBox()\n"
                "    val make: () -> LocalBox = ::LocalBox\n"
                "    return alternate(pick(first))\n"
                "}\n"
            ),
            "src/star.scoop": (
                "import renamerelay.*\nfun star(): Boolean = visible(pick(true))\n"
                "fun box(): VisibleBox = VisibleBox()\n"
            ),
            "src/package.scoop": "package renamerelay\nfun local(): Boolean = visible(true)\n",
            "src/types.scoop": "fun type(value: renamerelay.VisibleBox): Boolean = true\n",
        },
    )
    case.clean()
    case.query(facade, "as visible", "visible", "rename", error=-32803, newName="alternate")
    case.rename(facade, "as visible", "visible", "exposed", 5)
    assert "exposed as localChoice" in case.texts[main], case.texts[main]
    assert "localChoice(true)" in case.texts[main], case.texts[main]
    case.rename(main, "::localChoice", "localChoice", "selected", 3)
    case.rename(original, "class Box", "Box", "Crate", 2)
    case.rename(facade, "as VisibleBox", "VisibleBox", "ExportedBox", 5)
    case.rename(main, "::LocalBox", "LocalBox", "LocalCrate", 5)
    case.rename(original, "fun pick", "pick", "choose", 7)
    assert "renameorigin.choose as alternate" in case.texts[facade], case.texts[facade]
    assert "alternate(choose(first))" in case.texts[main], case.texts[main]
    assert "selected(true)" in case.texts[main], case.texts[main]
    case.finish()
