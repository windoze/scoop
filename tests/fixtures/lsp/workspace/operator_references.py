"""Explicit operator tokens navigate through the ordinary selected callable."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/operators.scoop"
    library = (
        "package operatorapi\n"
        "public class OperatorValue<T> {\n"
        "    /// Returns the input.\n"
        "    public operator fun plus(other: T): T = other\n"
        "    /// Orders two values.\n"
        "    public operator fun compareTo(other: OperatorValue<T>): Long = 0L\n"
        "    /// Tests equality.\n"
        "    public operator fun equals(other: OperatorValue<T>): Boolean = true\n"
        "    /// Negates the value.\n"
        "    public operator fun unaryMinus(): OperatorValue<T> = this\n"
        "}\n"
        "public class OperatorReceiver\n"
        "/// Combines the input.\n"
        "public operator fun OperatorReceiver.plus(other: Boolean): Boolean = other\n"
    )
    path = root / "src/operators.scoop"
    source = (
        "import operatorapi.OperatorValue as OperatorBox\n"
        "import operatorapi.OperatorReceiver\n"
        "import operatorapi.plus as combineOperator\n"
        "class OperatorLocal(seed: OperatorBox<Boolean>) { val ready: Boolean = seed /* 🌟 */ + true }\n"
        "class OperatorBag {\n"
        "    /// Tests membership.\n"
        "    operator fun contains(value: Boolean): Boolean = value\n"
        "}\n"
        "fun operatorUse(left: OperatorBox<Boolean>, right: OperatorBox<Boolean>, bag: OperatorBag, extension: OperatorReceiver): Boolean {\n"
        "    val one = left + true\n"
        "    val named = left.plus(true)\n"
        "    val negated = -left\n"
        "    val lower = left < right\n"
        "    val greater = left >= right\n"
        "    val inside = true in bag\n"
        "    val outside = false !in bag\n"
        "    val branched = when (true) { in bag -> true; !in bag -> false; else -> false }\n"
        "    val combined = extension + true\n"
        "    return left != right\n"
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

    def target(file, text, needle, token):
        start = position(text, needle, token)
        return {
            "uri": file.as_uri(),
            "range": {
                "start": start,
                "end": {**start, "character": start["character"] + len(token)},
            },
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

    def hover(needle, token, signature, documentation):
        result = query(needle, token, "hover")
        assert result, (needle, token, result)
        assert result["contents"] == [{"language": "scoop", "value": signature}, documentation], (
            result
        )

    def refs(needle, token):
        return query(needle, token, "references", context={"includeDeclaration": True})

    assert published(peer, path.as_uri(), 1) == []
    for needle, token, name, signature, documentation in [
        ("left + true", "+", "plus", "fun plus(other: T): T", "Returns the input."),
        ("-left", "-", "unaryMinus", "fun unaryMinus(): OperatorValue<T>", "Negates the value."),
        (
            "left < right",
            "<",
            "compareTo",
            "fun compareTo(other: OperatorValue<T>): Long",
            "Orders two values.",
        ),
        (
            "left >= right",
            ">=",
            "compareTo",
            "fun compareTo(other: OperatorValue<T>): Long",
            "Orders two values.",
        ),
        (
            "left != right",
            "!=",
            "equals",
            "fun equals(other: OperatorValue<T>): Boolean",
            "Tests equality.",
        ),
    ]:
        hover(needle, token, signature, documentation)
        assert query(needle, token, "definition") == target(provider, library, f"fun {name}(", name)
    hover("seed /* 🌟 */ + true", "+", "fun plus(other: T): T", "Returns the input.")
    assert len(refs("left + true", "+")) == 4
    assert len(refs("left < right", "<")) == 3
    assert len(refs("left != right", "!=")) == 2

    for needle, token in [
        ("true in bag", "in"),
        ("false !in bag", "!in"),
        ("{ in bag", "in"),
        ("; !in bag", "!in"),
    ]:
        hover(needle, token, "fun contains(value: Boolean): Boolean", "Tests membership.")
        assert query(needle, token, "definition") == target(
            path, source, "fun contains(", "contains"
        )
        assert len(refs(needle, token)) == 5
    hover(
        "extension + true",
        "+",
        "fun OperatorReceiver.plus(other: Boolean): Boolean",
        "Combines the input.",
    )
    assert query("extension + true", "+", "definition") == target(
        provider, library, "OperatorReceiver.plus", "plus"
    )
    assert len(refs("plus as combineOperator", "plus")) == 3
    assert len(refs("combineOperator", "combineOperator")) == 2
    assert len(refs("extension + true", "+")) == 2

    changed_library = library.replace("Returns the input.", "Returns the updated input.")
    change(peer, provider.as_uri(), 2, changed_library)
    hover("left + true", "+", "fun plus(other: T): T", "Returns the updated input.")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    hover("left + true", "+", "fun plus(other: T): T", "Returns the input.")

    original = source
    source = source.replace("val one = left + true", "val one = true")
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(refs("left.plus(true)", "plus")) == 3
    source += 'fun operatorBroken(value: OperatorBox<Boolean>) { val bad = value + "wrong" }\n'
    change(peer, path.as_uri(), 3, source)
    assert published(peer, path.as_uri(), 3)
    assert query('value + "wrong"', "+", "hover") is None
    assert len(refs("left.plus(true)", "plus")) == 3
    hover("left < right", "<", "fun compareTo(other: OperatorValue<T>): Long", "Orders two values.")

    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(refs("left + true", "+")) == 4
    hover("left + true", "+", "fun plus(other: T): T", "Returns the input.")
