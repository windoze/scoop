"""References follow real export bindings across source Cone re-export paths."""

from diagnostics import change, published


def check(peer, root):
    project = root / "reexport-project"
    texts = {
        "origin/src/api.scoop": (
            "package reexportorigin\n"
            "public fun pick(value: Boolean): Boolean = value\n"
            "public fun other(value: Boolean): Boolean = value\n"
            "public class Box public constructor()\n"
        ),
        "facade/src/api.scoop": (
            "package reexportfacade\n"
            "public import reexportorigin.pick as visible\n"
            "public import reexportorigin.pick as alternate\n"
            "public import reexportorigin.Box as VisibleBox\n"
            "fun facadeUse(): Boolean = visible(true)\n"
        ),
        "relay/src/api.scoop": "package reexportrelay\npublic import reexportfacade.*\n",
        "src/main.scoop": (
            "import reexportrelay.visible as localChoice\n"
            "import reexportrelay.alternate\n"
            "import reexportrelay.VisibleBox as LocalBox\n"
            "fun reexportUse(value: LocalBox): Boolean {\n"
            "    val first = /* 🌟 */ localChoice(true)\n"
            "    val factory: (Boolean) -> Boolean = ::localChoice\n"
            "    val box = LocalBox()\n"
            "    val boxFactory: () -> LocalBox = ::LocalBox\n"
            "    return alternate(first)\n"
            "}\n"
        ),
        "src/star.scoop": (
            "import reexportrelay.*\nfun reexportStar(): Boolean = visible(alternate(true))\n"
            "fun reexportStarBox() { val box = VisibleBox() }\n"
        ),
        "src/package.scoop": (
            "package reexportrelay\nfun reexportPackage(): Boolean = visible(true)\n"
            "fun reexportPackageBox() { val box = VisibleBox() }\n"
        ),
        "src/types.scoop": "fun reexportType(value: reexportrelay.VisibleBox): Boolean = true\n",
    }
    for directory, name, dependencies in [
        ("origin", "refs-origin", []),
        ("facade", "refs-facade", [("refs-origin", "../origin")]),
        ("relay", "refs-relay", [("refs-facade", "../facade")]),
        ("", "refs-consumer", [("refs-facade", "facade"), ("refs-relay", "relay")]),
    ]:
        cone = project / directory
        cone.mkdir(parents=True, exist_ok=True)
        manifest = (
            f'schema = 1\n[cone]\ngroup = "test"\nname = "{name}"\n'
            'version = "1.0.0"\nkind = "library"\n[dependencies]\n'
        )
        for dependency, path in dependencies:
            manifest += f'"test:{dependency}" = {{version = "1.0.0", path = "{path}"}}\n'
        (cone / "Cone.toml").write_text(manifest)
    for relative, text in texts.items():
        path = project / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    folder = {"name": "reexports", "uri": project.as_uri()}
    peer.resolve(project)
    peer.notify(
        "workspace/didChangeWorkspaceFolders", {"event": {"added": [folder], "removed": []}}
    )
    for relative, text in texts.items():
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": (project / relative).as_uri(),
                    "version": 1,
                    "languageId": "scoop",
                    "text": text,
                }
            },
        )

    def position(relative, needle, name):
        text = texts[relative]
        prefix = text[: text.index(needle) + needle.index(name)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def location(relative, needle, name):
        start = position(relative, needle, name)
        return (project / relative).as_uri(), start["line"], start["character"]

    def references(relative, needle, name, declarations=True):
        result = peer.request(
            "textDocument/references",
            {
                "textDocument": {"uri": (project / relative).as_uri()},
                "position": position(relative, needle, name),
                "context": {"includeDeclaration": declarations},
            },
        )
        assert result is not None, (relative, needle, peer.diagnostics)
        return {
            (item["uri"], item["range"]["start"]["line"], item["range"]["start"]["character"])
            for item in result
        }

    for relative in texts:
        assert published(peer, (project / relative).as_uri(), 1) == []
    facade = "facade/src/api.scoop"
    main = "src/main.scoop"
    original = "origin/src/api.scoop"
    expected = {
        location(facade, "as visible", "visible"),
        location(facade, "visible(true)", "visible"),
        location(main, "visible as localChoice", "visible"),
        location(main, "localChoice(true)", "localChoice"),
        location(main, "::localChoice", "localChoice"),
        location("src/star.scoop", "visible(alternate", "visible"),
        location("src/package.scoop", "visible(true)", "visible"),
    }
    actual = references(facade, "as visible", "visible")
    assert actual == expected, (actual - expected, expected - actual)
    assert references(main, "visible as localChoice", "visible") == {
        item for item in expected if item[0] != (project / facade).as_uri()
    }
    assert references(facade, "as visible", "visible", False) == expected - {
        location(facade, "as visible", "visible")
    }
    assert len(references(main, "as localChoice", "localChoice")) == 3
    assert len(references(facade, "as alternate", "alternate")) == 4
    assert len(references(original, "fun pick", "pick")) == 12
    assert references(facade, "as VisibleBox", "VisibleBox") == {
        location(facade, "as VisibleBox", "VisibleBox"),
        location(main, "VisibleBox as LocalBox", "VisibleBox"),
        location(main, "value: LocalBox", "LocalBox"),
        location(main, "LocalBox()", "LocalBox"),
        location(main, "() -> LocalBox", "LocalBox"),
        location(main, "::LocalBox", "LocalBox"),
        location("src/star.scoop", "VisibleBox()", "VisibleBox"),
        location("src/package.scoop", "VisibleBox()", "VisibleBox"),
        location("src/types.scoop", "reexportrelay.VisibleBox", "VisibleBox"),
    }

    saved = texts[facade]
    expected.remove(location(facade, "as visible", "visible"))
    texts[facade] = saved.replace(
        "reexportorigin.pick as visible", "reexportorigin.other as visible"
    )
    expected.add(location(facade, "as visible", "visible"))
    change(peer, (project / facade).as_uri(), 2, texts[facade])
    assert published(peer, (project / facade).as_uri(), 2) == []
    actual = references(facade, "as visible", "visible")
    assert actual == expected, (actual - expected, expected - actual)
    assert len(references(original, "fun pick", "pick")) == 5
    peer.notify("textDocument/didClose", {"textDocument": {"uri": (project / facade).as_uri()}})
    expected.remove(location(facade, "as visible", "visible"))
    texts[facade] = saved
    expected.add(location(facade, "as visible", "visible"))
    assert len(references(original, "fun pick", "pick")) == 12

    star = "src/star.scoop"
    saved = texts[star]
    texts[star] = saved.replace("visible(alternate(true))", "alternate(alternate(true))")
    change(peer, (project / star).as_uri(), 2, texts[star])
    assert published(peer, (project / star).as_uri(), 2) == []
    assert len(references(facade, "as visible", "visible")) == 6
    peer.notify("textDocument/didClose", {"textDocument": {"uri": (project / star).as_uri()}})
    texts[star] = saved
    assert references(facade, "as visible", "visible") == expected
    for relative in texts.keys() - {facade, star}:
        peer.notify(
            "textDocument/didClose", {"textDocument": {"uri": (project / relative).as_uri()}}
        )
    peer.notify(
        "workspace/didChangeWorkspaceFolders", {"event": {"added": [], "removed": [folder]}}
    )
