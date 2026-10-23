"""Static unit variants share declaration identities and applied type queries."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/static_variants.scoop"
    library = (
        "package staticvariantapi\n"
        "public enum Choice<T> {\n"
        "    /// The empty variant.\n"
        "    Empty,\n"
        "    /// The other variant.\n"
        "    Other,\n"
        "    Item(val value: T)\n"
        "}\n"
    )
    path = root / "src/static_variants.scoop"
    source = (
        "import staticvariantapi.Choice as StaticChoice\n"
        "import staticvariantapi.Choice.Empty as staticEmpty\n"
        "typealias StaticAlias<T> = StaticChoice<T>\n"
        "val staticFirst: StaticChoice<Boolean> = staticEmpty\n"
        "val staticSecond: StaticChoice<String> = StaticAlias<String>.Empty\n"
        "val staticThird: StaticChoice<Boolean> = StaticChoice<Boolean>.Other\n"
        "fun staticVariantRead(): StaticChoice<Boolean> = staticEmpty\n"
    )
    for file, text in [(provider, library), (path, source)]:
        file.write_text(text)
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": file.as_uri(),
                    "languageId": "scoop",
                    "version": 1,
                    "text": text,
                }
            },
        )

    def position(text, needle, name):
        prefix = text[: text.index(needle) + needle.index(name)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, name, method, **extra):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": position(source, needle, name),
                **extra,
            },
        )

    def target(file, text, needle, name):
        start = position(text, needle, name)
        end = {**start, "character": start["character"] + len(name)}
        return {"uri": file.as_uri(), "range": {"start": start, "end": end}}

    def alias_refs():
        return query(
            "Empty as staticEmpty",
            "staticEmpty",
            "references",
            context={"includeDeclaration": True},
        )

    assert published(peer, path.as_uri(), 1) == []
    empty = target(provider, library, "Empty,", "Empty")
    choice = target(provider, library, "enum Choice", "Choice")
    for needle, name, label in [
        ("Boolean> = staticEmpty", "staticEmpty", "Empty: Choice<Boolean>"),
        ("StaticAlias<String>.Empty", "Empty", "Empty: Choice<String>"),
    ]:
        hover = query(needle, name, "hover")
        assert hover["contents"][0]["value"] == label, hover
        assert hover["contents"][1] == "The empty variant.", hover
        assert query(needle, name, "definition") == empty
        assert query(needle, name, "typeDefinition") == choice
    assert query("StaticAlias<String>", "StaticAlias", "typeDefinition") == choice
    qualifier = query("StaticAlias<String>", "StaticAlias", "hover")
    assert "Choice<String>" in str(qualifier["contents"]), qualifier
    aliases = query(
        "StaticAlias<String>", "StaticAlias", "references", context={"includeDeclaration": True}
    )
    assert len(aliases) == 2, aliases
    assert len(alias_refs()) == 3
    originals = query(
        "StaticAlias<String>.Empty", "Empty", "references", context={"includeDeclaration": True}
    )
    assert len(originals) == 5 and empty in originals, originals

    original_source = source
    source = source.replace(
        "staticFirst: StaticChoice<Boolean> = staticEmpty",
        "staticFirst: StaticChoice<Boolean> = StaticChoice<Boolean>.Other",
    )
    source = source.replace("StaticChoice<String>", "StaticChoice<Boolean>").replace(
        "StaticAlias<String>", "StaticAlias<Boolean>"
    )
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(alias_refs()) == 2
    hover = query("StaticAlias<Boolean>.Empty", "Empty", "hover")
    assert hover["contents"][0]["value"] == "Empty: Choice<Boolean>", hover

    source += "val staticBroken: String = StaticChoice<Boolean>.Empty\n"
    change(peer, path.as_uri(), 3, source)
    errors = published(peer, path.as_uri(), 3)
    assert errors, errors
    assert query("staticBroken: String = StaticChoice<Boolean>.Empty", "Empty", "hover") is None
    refs = query(
        "StaticAlias<Boolean>.Empty", "Empty", "references", context={"includeDeclaration": True}
    )
    assert len(refs) == 4, refs

    source = original_source
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(alias_refs()) == 3
    original_library = library
    library = library.replace("The empty variant.", "The updated empty variant.")
    change(peer, provider.as_uri(), 2, library)
    assert published(peer, provider.as_uri(), 2) == []
    hover = query("StaticAlias<String>.Empty", "Empty", "hover")
    assert hover["contents"][1] == "The updated empty variant.", hover
    library = original_library
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    hover = query("StaticAlias<String>.Empty", "Empty", "hover")
    assert hover["contents"][1] == "The empty variant.", hover
