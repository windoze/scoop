"""Static numeric calls share bindings, arguments and descriptions with source bodies."""

from diagnostics import change, published


def check(peer, root):
    path = root / "src/static_calls.scoop"
    source = (
        "/// 数值调用 🌟\n"
        "const val numericCallShift: Int = 3.shl(count = 1L)\n"
        "const val numericCallInfix: Int = 4 shl 2L\n"
        "val numericCallImage: Int = 5.shl(count = 3L)\n"
        "const val numericCallNested: Double = 2.plus(other = 3).toDouble()\n"
        "const val numericCallFraction: Double = (1.5).plus(other = 2.5)\n"
        "val numericCallFloatImage: Double = (8.0).div(other = 2.0)\n"
        "fun numericCallRun(number: Int, distance: Long): Int = number.shl(count = distance)\n"
        "fun numericCallInput(): Int = 1\n"
        "val numericCallRuntime: Int = 7.plus(other = 8) + numericCallInput()\n"
    )
    path.write_text(source)
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": path.as_uri(),
                "languageId": "scoop",
                "version": 1,
                "text": source,
            }
        },
    )

    def position(needle, token):
        prefix = source[: source.index(needle) + needle.index(token)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, token, method, **extra):
        return peer.request(
            f"textDocument/{method}",
            {"textDocument": {"uri": path.as_uri()}, "position": position(needle, token), **extra},
        )

    def label(needle, token):
        result = query(needle, token, "hover")
        assert result, (needle, token, result)
        contents = result["contents"]
        if isinstance(contents, list):
            contents = contents[0]
        return contents["value"]

    def refs(needle, token):
        result = query(needle, token, "references", context={"includeDeclaration": True})
        return [location for location in result if location["uri"] == path.as_uri()]

    def signature(needle, token):
        result = query(needle, token, "signatureHelp")
        assert result, (needle, result)
        return result["signatures"][0]["label"]

    assert published(peer, path.as_uri(), 1) == []
    for needle in ["3.shl", "4 shl", "5.shl"]:
        assert "shl(count: Long): Int" in label(needle, "shl")
        assert query(needle, "shl", "definition") == query("number.shl", "shl", "definition")
    assert label("count = 1L", "count") == "count: Long"
    assert query("count = 1L", "count", "definition") == query(
        "count = distance", "count", "definition"
    )
    assert query("count = 1L", "count", "typeDefinition") == query(
        "distance: Long", "distance", "typeDefinition"
    )
    assert signature("3.shl(count", "count") == "shl(count: Long): Int"
    assert signature("2.plus(other", "other") == "plus(other: Int): Int"
    assert signature("toDouble()", ")") == "toDouble(): Double"
    assert signature("(1.5).plus(other", "other") == "plus(other: Double): Double"
    assert signature("(8.0).div(other", "other") == "div(other: Double): Double"
    assert label("(1.5).plus(other", "other") == "other: Double"
    assert len(refs("3.shl", "shl")) == 4
    assert len(refs("count = 1L", "count")) == 3
    assert len(refs("2.plus", "plus")) == 3
    assert query("+ numericCallInput()", "+", "definition") == query("7.plus", "plus", "definition")
    assert len(refs("other = 3", "other")) == 2

    original = source
    source = source.replace("3.shl(count = 1L)", "3.shr(count = 1L)")
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert signature("3.shr(count", "count") == "shr(count: Long): Int"
    assert len(refs("number.shl", "shl")) == 3
    assert len(refs("3.shr", "shr")) == 1

    source += "const val numericCallInvalid: Int = 2.div(other = 0)\n"
    change(peer, path.as_uri(), 3, source)
    assert published(peer, path.as_uri(), 3)
    assert query("2.div(other = 0)", "div", "hover") is None
    assert query("2.div(other = 0)", "other", "signatureHelp") is None
    assert signature("3.shr(count", "count") == "shr(count: Long): Int"
    assert len(refs("2.plus", "plus")) == 3

    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert signature("3.shl(count", "count") == "shl(count: Long): Int"
    assert len(refs("number.shl", "shl")) == 4
