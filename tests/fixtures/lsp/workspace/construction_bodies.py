"""Constructor plans share source facts with the ordinary frontend."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/construction.scoop"
    library = (
        "package constructionapi\n"
        "/// @param stored The root value.\n"
        "public open class ConstructionRoot<T> public constructor(public var stored: T)\n"
        "/// Pass through the input.\n"
        "/// @param input The supplied value.\n"
        "public fun <T> pick(input: T): T = input\n"
    )
    path = root / "src/construction.scoop"
    source = (
        "import constructionapi.ConstructionRoot as ConstructionBase\n"
        "import constructionapi.pick as constructionPick\n"
        "/// 构造区域 🌟\n"
        "/// @param seed The original seed.\n"
        "class ConstructionBox<T>(seed: T): ConstructionBase<T>(stored = constructionPick(input = seed)) {\n"
        "    val snapshot: T = stored\n"
        "    init { val read = constructionPick(input = stored) }\n"
        "    /// @param copy The secondary input.\n"
        "    constructor(copy: T, marker: Boolean): this(seed = copy) { val used = constructionPick(input = copy) }\n"
        "}\n"
        "class ConstructionTerminal: ConstructionBase<Boolean> {\n"
        "    constructor(value: Boolean): super(stored = value) {}\n"
        "}\n"
        "class ConstructionPayload(val flag: Boolean)\n"
        "/// @param source The payload.\n"
        "class ConstructionReader(source: ConstructionPayload) { val ready: Boolean = source.flag }\n"
        "struct ConstructionValue(val value: Boolean) {\n"
        "    /// @param seed The text seed.\n"
        "    constructor(seed: String): this(value = true) { val echo = constructionPick(input = seed) }\n"
        "}\n"
        "object ConstructionState {\n"
        "    val active: Boolean = constructionPick(input = true)\n"
        "    init { val copy = constructionPick(input = active) }\n"
        "}\n"
        "fun constructionLocal(flag: Boolean): Boolean {\n"
        "    val captured = flag\n"
        "    class Local { val stored: Boolean = constructionPick(input = captured) }\n"
        "    return Local().stored\n"
        "}\n"
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

    def position(text, needle, token):
        prefix = text[: text.index(needle) + needle.index(token)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, token, method, **extra):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": position(source, needle, token),
                **extra,
            },
        )

    def target(file, text, needle, token):
        start = position(text, needle, token)
        return {
            "uri": file.as_uri(),
            "range": {
                "start": start,
                "end": {**start, "character": start["character"] + len(token)},
            },
        }

    def hover(needle, token, label, documentation=None):
        result = query(needle, token, "hover")
        assert result, (needle, token, result)
        contents = result["contents"]
        assert (contents[0] if isinstance(contents, list) else contents)["value"] == label, result
        if documentation is not None:
            assert isinstance(contents, list) and contents[1] == documentation, result

    def refs(needle, token):
        return query(needle, token, "references", context={"includeDeclaration": True})

    assert published(peer, path.as_uri(), 1) == []
    hover("input = seed))", "seed", "seed: T", "The original seed.")
    assert query("input = seed))", "seed", "definition") == target(
        path, source, "ConstructionBox<T>(seed", "seed"
    )
    assert len(refs("input = seed))", "seed")) == 3
    hover("input = seed))", "input", "input: T", "The supplied value.")
    hover("stored = constructionPick", "stored", "stored: T", "The root value.")
    assert query("stored = constructionPick", "stored", "definition") == target(
        provider, library, "var stored", "stored"
    )
    signature = query("ConstructionBase<T>(stored", "stored", "signatureHelp")
    assert (
        signature["signatures"][0]["label"] == "ConstructionBase(stored: T): ConstructionRoot<T>"
    ), signature
    assert len(refs("constructionPick(input = seed)", "constructionPick")) == 8

    hover("snapshot: T = stored", "stored", "var stored: T")
    assert query("snapshot: T = stored", "stored", "definition") == target(
        provider, library, "var stored", "stored"
    )
    assert len(refs("snapshot: T = stored", "stored")) == 3
    assert len(refs("stored = constructionPick", "stored")) == 3
    hover("input = copy)", "copy", "copy: T", "The secondary input.")
    assert query("input = copy)", "copy", "definition") == target(
        path, source, "constructor(copy", "copy"
    )
    signature = query("this(seed = copy)", "seed", "signatureHelp")
    assert signature["signatures"][0]["label"] == "this(seed: T): ConstructionBox<T>", signature
    assert query("this(seed = copy)", "this", "definition") == target(
        path, source, "class ConstructionBox", "ConstructionBox"
    )
    assert query("super(stored = value)", "super", "definition") == target(
        provider, library, "class ConstructionRoot", "ConstructionRoot"
    )
    signature = query("super(stored = value)", "stored", "signatureHelp")
    assert (
        signature["signatures"][0]["label"] == "super(stored: Boolean): ConstructionRoot<Boolean>"
    ), signature

    hover("source.flag", "source", "source: ConstructionPayload", "The payload.")
    assert query("source.flag", "source", "typeDefinition") == target(
        path, source, "class ConstructionPayload", "ConstructionPayload"
    )
    completion = query("source.flag", "flag", "completion")
    assert any(item["label"] == "flag" for item in completion["items"]), completion
    hover("input = seed) }", "seed", "seed: String", "The text seed.")
    assert query("input = seed) }", "seed", "definition") == target(
        path, source, "constructor(seed: String)", "seed"
    )
    assert len(refs("input = seed) }", "seed")) == 2
    hover("input = active)", "active", "val active: Boolean")
    assert query("input = active)", "active", "definition") == target(
        path, source, "val active", "active"
    )
    assert len(refs("input = active)", "active")) == 2
    hover("input = captured)", "captured", "captured: Boolean")
    assert query("input = captured)", "captured", "definition") == target(
        path, source, "val captured", "captured"
    )
    assert len(refs("input = captured)", "captured")) == 2

    original = source
    source = source.replace("constructionPick(input = seed))", "seed)")
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(refs("constructionPick(input = copy)", "constructionPick")) == 7
    source += (
        "class ConstructionInvalid { val failed = missingConstruction; val healthy: Boolean = constructionPick(input = true); fun independent(): Boolean = true }\n"
        "class ConstructionMissing: ConstructionBase<Boolean>\n"
        'class ConstructionCycle { constructor(value: Boolean): this("x") {}; constructor(value: String): this(true) {} }\n'
    )
    change(peer, path.as_uri(), 3, source)
    diagnostics = published(peer, path.as_uri(), 3)
    assert any("missingConstruction" in diagnostic["message"] for diagnostic in diagnostics), (
        diagnostics
    )
    assert any(
        diagnostic["message"]
        == "dependency constructor `ConstructionRoot` expects 1 argument(s), but 0 were supplied"
        for diagnostic in diagnostics
    ), diagnostics
    assert any(
        "constructor delegation cycle" in diagnostic["message"] for diagnostic in diagnostics
    ), diagnostics
    assert query("missingConstruction", "missingConstruction", "hover") is None
    hover(
        "healthy: Boolean = constructionPick",
        "constructionPick",
        "fun <T> pick(input: T): T",
        "Pass through the input.\n\n**@param** input — The supplied value.",
    )
    hover("fun independent()", "independent", "fun independent(): Boolean")

    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(refs("constructionPick(input = seed)", "constructionPick")) == 8
    hover("input = seed))", "seed", "seed: T", "The original seed.")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
