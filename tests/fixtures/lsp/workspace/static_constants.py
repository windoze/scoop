"""Constant reads reuse the original evaluator's bindings and source positions."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/constants.scoop"
    library = (
        "package constantapi\n"
        "/// The shared constant.\n"
        "public const val FLAG: Boolean = true\n"
        "public object Holder {\n"
        "    /// The member constant.\n"
        "    public const val FLAG: Boolean = false\n"
        "}\n"
    )
    path = root / "src/constants.scoop"
    source = (
        "import constantapi.FLAG as constantEnabled\n"
        "import constantapi.Holder.FLAG as constantMember\n"
        "import constantapi.Holder as ConstantHolder\n"
        "const val constantDerived = constantEnabled\n"
        "val constantImage: Boolean = !constantEnabled\n"
        "const val constantNested = ConstantHolder.FLAG\n"
        "val constantObjectImage: Boolean = constantMember\n"
        "const val constantSeed: Boolean = true\n"
        "const val constantForward = constantLast\n"
        "const val constantLast = constantSeed\n"
        "val constantStatic: Boolean = constantLast && constantSeed\n"
        "object ConstantShadow {\n"
        "    const val constantEnabled = false\n"
        "    const val copied = constantEnabled\n"
        "}\n"
        "fun constantRuntimeValue(): Boolean = true\n"
        "val constantRuntime: Boolean = constantSeed && constantRuntimeValue()\n"
        "const val 常量标记: Boolean = true\n"
        "val constantChinese: Boolean = 常量标记\n"
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
        end = {**start, "character": start["character"] + len(name.encode("utf-16-le")) // 2}
        return {"uri": file.as_uri(), "range": {"start": start, "end": end}}

    def refs(needle, name):
        return query(needle, name, "references", context={"includeDeclaration": True})

    def alias_refs():
        return refs("FLAG as constantEnabled", "constantEnabled")

    assert published(peer, path.as_uri(), 1) == []
    original = target(provider, library, "const val FLAG: Boolean = true", "FLAG")
    for needle in ["constantDerived = constantEnabled", "!constantEnabled"]:
        hover = query(needle, "constantEnabled", "hover")
        assert hover["contents"][0]["value"] == "val FLAG: Boolean", hover
        assert hover["contents"][1] == "The shared constant.", hover
        assert query(needle, "constantEnabled", "definition") == original
    member = target(provider, library, "const val FLAG: Boolean = false", "FLAG")
    assert query("ConstantHolder.FLAG", "FLAG", "definition") == member
    assert query("Boolean = constantMember", "constantMember", "definition") == member
    assert len(alias_refs()) == 3
    assert len(refs("FLAG as constantMember", "constantMember")) == 2
    assert len(refs("constantSeed:", "constantSeed")) == 4
    last = target(path, source, "const val constantLast", "constantLast")
    assert query("constantForward = constantLast", "constantLast", "definition") == last
    assert len(refs("constantLast =", "constantLast")) == 3
    shadow = target(path, source, "constantEnabled = false", "constantEnabled")
    assert query("copied = constantEnabled", "constantEnabled", "definition") == shadow
    assert len(refs("copied = constantEnabled", "constantEnabled")) == 2
    chinese = query("Boolean = 常量标记", "常量标记", "definition")
    assert chinese == target(path, source, "常量标记:", "常量标记"), chinese
    assert len(refs("Boolean = 常量标记", "常量标记")) == 2

    original_source = source
    source = source.replace(
        "constantImage: Boolean = !constantEnabled", "constantImage: Boolean = true"
    )
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(alias_refs()) == 2

    source = source.replace("constantDerived = constantEnabled", "constantDerived = constantMember")
    change(peer, path.as_uri(), 3, source)
    assert published(peer, path.as_uri(), 3) == []
    assert len(alias_refs()) == 1
    assert len(refs("FLAG as constantMember", "constantMember")) == 3
    hover = query("constantDerived = constantMember", "constantMember", "hover")
    assert hover["contents"][1] == "The member constant.", hover

    source = original_source
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(alias_refs()) == 3
    original_library = library
    library = library.replace("The shared constant.", "The updated constant.").replace(
        "FLAG: Boolean = true", "FLAG: Boolean = false"
    )
    change(peer, provider.as_uri(), 2, library)
    assert published(peer, provider.as_uri(), 2) == []
    hover = query("!constantEnabled", "constantEnabled", "hover")
    assert hover["contents"][1] == "The updated constant.", hover
    library = original_library
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    hover = query("!constantEnabled", "constantEnabled", "hover")
    assert hover["contents"][1] == "The shared constant.", hover
