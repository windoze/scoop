"""Definition-site defaults share body queries and parameter identities."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/defaults.scoop"
    library = (
        "package defaultapi\n"
        "/// The default helper.\n"
        "/// @param input The supplied flag.\n"
        "public fun pick(input: Boolean): Boolean = input\n"
    )
    path = root / "src/defaults.scoop"
    source = (
        "import defaultapi.pick as defaultPick\n"
        "/// 默认参数 🌟\n"
        "/// @param seed The original flag.\n"
        "fun defaultUnused(seed: Boolean, copy: Boolean = defaultPick(input = seed)): Boolean = copy\n"
        "/// @param initial The class seed.\n"
        "class DefaultBox<T>(initial: T, val stored: T = initial)\n"
        "/// @param initial The struct seed.\n"
        "struct DefaultValue<T>(val initial: T, val stored: T = initial)\n"
        "enum DefaultEvent<T> {\n"
        "    /// @param initial The variant seed.\n"
        "    Item(val initial: T, val stored: T = initial)\n"
        "}\n"
        "class DefaultPayload(val flag: Boolean)\n"
        "/// @param seed The captured payload.\n"
        "fun defaultOuter(seed: DefaultPayload): Boolean {\n"
        "    fun local(value: Boolean = seed.flag): Boolean = value\n"
        "    return local()\n"
        "}\n"
        "fun defaultLocalType(seed: Boolean): Boolean {\n"
        "    class Local(val value: Boolean = seed)\n"
        "    return Local().value\n"
        "}\n"
        "fun defaultLambda(seed: Boolean, value: () -> Boolean = { seed }): Boolean = value()\n"
        "fun defaultVararg(seed: Boolean, vararg rest: Boolean = [seed]): Boolean = seed\n"
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

    def position(text, needle, token, offset=0):
        prefix = text[: text.index(needle) + needle.index(token) + offset]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, token, method, offset=0, **extra):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": position(source, needle, token, offset),
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
        if documentation is None:
            assert (contents[0] if isinstance(contents, list) else contents)["value"] == label, (
                result
            )
        else:
            assert contents[0]["value"] == label and contents[1] == documentation, result

    def refs(needle, token):
        return query(needle, token, "references", context={"includeDeclaration": True})

    assert published(peer, path.as_uri(), 1) == []
    hover("input = seed", "seed", "seed: Boolean", "The original flag.")
    assert query("input = seed", "seed", "definition") == target(
        path, source, "defaultUnused(seed", "seed"
    )
    assert len(refs("input = seed", "seed")) == 2
    hover("input = seed", "input", "input: Boolean", "The supplied flag.")
    assert query("defaultPick(input", "defaultPick", "definition") == target(
        provider, library, "fun pick", "pick"
    )
    signature = query("defaultPick(input", "input", "signatureHelp")
    assert signature["signatures"][0]["label"] == "defaultPick(input: Boolean): Boolean", signature
    assert len(refs("defaultPick(input", "defaultPick")) == 2

    for needle, declaration, documentation in [
        (
            "DefaultBox<T>(initial: T, val stored: T = initial)",
            "DefaultBox<T>(initial",
            "The class seed.",
        ),
        (
            "DefaultValue<T>(val initial: T, val stored: T = initial)",
            "DefaultValue<T>(val initial",
            "The struct seed.",
        ),
        ("Item(val initial: T, val stored: T = initial)", "Item(val initial", "The variant seed."),
    ]:
        offset = needle.rindex("initial") - needle.index("initial")
        result = query(needle, "initial", "hover", offset)
        assert result["contents"][0]["value"] == "initial: T", result
        assert result["contents"][1] == documentation, result
        origin = target(path, source, declaration, "initial")
        assert query(needle, "initial", "definition", offset) == origin
        references = query(
            needle, "initial", "references", offset, context={"includeDeclaration": True}
        )
        assert len(references) == 2 and origin in references, references

    hover("seed.flag", "seed", "seed: DefaultPayload", "The captured payload.")
    assert query("seed.flag", "seed", "definition") == target(
        path, source, "defaultOuter(seed", "seed"
    )
    assert query("seed.flag", "seed", "typeDefinition") == target(
        path, source, "class DefaultPayload", "DefaultPayload"
    )
    assert len(refs("seed.flag", "seed")) == 2
    completion = query("seed.flag", "flag", "completion")
    assert any(item["label"] == "flag" for item in completion["items"]), completion
    for needle, declaration, count in [
        ("Boolean = seed)", "defaultLocalType(seed", 2),
        ("{ seed }", "defaultLambda(seed", 2),
        ("[seed]", "defaultVararg(seed", 3),
    ]:
        hover(needle, "seed", "seed: Boolean")
        assert query(needle, "seed", "definition") == target(path, source, declaration, "seed")
        assert len(refs(needle, "seed")) == count

    original = source
    source = source.replace("defaultPick(input = seed)", "defaultPick(input = false)")
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(refs("defaultUnused(seed", "seed")) == 1
    source += "fun defaultInvalid(value: Boolean = missingDefault): Boolean = value\n"
    change(peer, path.as_uri(), 3, source)
    diagnostics = published(peer, path.as_uri(), 3)
    assert any("missingDefault" in diagnostic["message"] for diagnostic in diagnostics), diagnostics
    assert query("missingDefault", "missingDefault", "hover") is None
    hover("seed.flag", "seed", "seed: DefaultPayload", "The captured payload.")

    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(refs("input = seed", "seed")) == 2
    hover("input = seed", "seed", "seed: Boolean", "The original flag.")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
