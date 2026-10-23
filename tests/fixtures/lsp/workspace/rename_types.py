"""Type, constructor, field and type parameter names keep their original bindings."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-types"
    manifest(project, "rename-types")
    file = "src/types.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            file: (
                "package renametypes\n"
                "struct Box<T>(val value: T)\n"
                "typealias Alias<A> = Box<A>\n"
                "fun exercise(): Boolean {\n"
                "    val direct = Box(value = true)\n"
                "    val alias = /* 🌟 */ Alias(value = false)\n"
                "    val factory: (Boolean) -> Alias<Boolean> = ::Alias\n"
                "    return direct.value\n"
                "}\n"
            )
        },
    )
    case.clean()
    case.rename(file, "Box(value", "Box", "Crate", 3)
    case.rename(file, "::Alias", "Alias", "Wrapped", 4)
    assert "Crate<A>" in case.texts[file], case.texts[file]
    case.rename(file, "<T>", "T", "Item", 2)
    case.rename(file, "<A>", "A", "Element", 2)
    case.rename(file, "val value", "value", "item", 4)
    assert "direct.item" in case.texts[file], case.texts[file]
    case.rename(file, "val direct", "direct", "local", 2)
    case.finish()
