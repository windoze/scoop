"""Subscript updates retain every selected call and its precise source token."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/subscripts.scoop"
    library = (
        "package subscriptapi\n"
        "public class SubscriptStep<T>(val tag: T) {\n"
        "    /// Advances the step.\n"
        "    public operator fun inc(): SubscriptStep<T> = this\n"
        "    /// Rewinds the step.\n"
        "    public operator fun dec(): SubscriptStep<T> = this\n"
        "    /// Adds the step.\n"
        "    public operator fun plus(other: SubscriptStep<T>): SubscriptStep<T> = this\n"
        "}\n"
        "public class SubscriptCell<T>(var item: T) {\n"
        "    /// Reads a cell.\n"
        "    public operator fun get(index: Boolean): T = item\n"
        "    /// Writes a cell.\n"
        "    public operator fun set(index: Boolean, value: T) { item = value }\n"
        "}\n"
        "public class SubscriptAmount {\n"
        "    /// Mutates the amount.\n"
        "    public operator fun plusAssign(other: SubscriptAmount) {}\n"
        "}\n"
        "public class SubscriptReadCell(val item: SubscriptAmount) {\n"
        "    public operator fun get(index: Boolean): SubscriptAmount = item\n"
        "}\n"
        "public class SubscriptAliasCell(var item: SubscriptStep<Boolean>)\n"
        "/// Reads an alias cell.\n"
        "public operator fun SubscriptAliasCell.get(index: Boolean): SubscriptStep<Boolean> = item\n"
        "/// Writes an alias cell.\n"
        "public operator fun SubscriptAliasCell.set(index: Boolean, value: SubscriptStep<Boolean>) { item = value }\n"
    )
    path = root / "src/subscripts.scoop"
    source = (
        "import subscriptapi.SubscriptStep\n"
        "import subscriptapi.SubscriptCell\n"
        "import subscriptapi.SubscriptAmount\n"
        "import subscriptapi.SubscriptReadCell\n"
        "import subscriptapi.SubscriptAliasCell\n"
        "import subscriptapi.get as readSubscript\n"
        "import subscriptapi.set as writeSubscript\n"
        "class SubscriptInitial(seed: SubscriptCell<Boolean>) { val ready = seed[true] }\n"
        "fun subscriptUse(cell: SubscriptCell<SubscriptStep<Boolean>>, value: SubscriptStep<Boolean>, alias: SubscriptAliasCell, readonly: SubscriptReadCell, amount: SubscriptAmount) {\n"
        "    val read = /* 🌟 */ cell[true]\n"
        "    cell[false] = value\n"
        "    val prefix = ++cell[true]\n"
        "    val postfix = cell[false]--\n"
        "    cell[true] += value\n"
        "    alias[true]++\n"
        "    readonly[true] += amount\n"
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

    def refs(needle, token):
        return query(needle, token, "references", context={"includeDeclaration": True})

    def locations(values):
        return {
            (value["uri"], value["range"]["start"]["line"], value["range"]["start"]["character"])
            for value in values
        }

    get = target(provider, library, "get(index: Boolean): T", "get")
    set_ = target(provider, library, "set(index: Boolean, value: T)", "set")
    get_signature = {"language": "scoop", "value": "fun get(index: Boolean): T"}
    set_signature = {"language": "scoop", "value": "fun set(index: Boolean, value: T): Unit"}
    assert published(peer, path.as_uri(), 1) == []
    assert query("/* 🌟 */ cell[true]", "[", "definition") == get
    assert query("cell[false] = value", "[", "definition") == set_
    assert query("seed[true]", "[", "definition") == get
    assert query("/* 🌟 */ cell[true]", "[", "hover")["contents"] == [
        get_signature,
        "Reads a cell.",
    ]
    assert query("cell[false] = value", "[", "hover")["contents"] == [
        set_signature,
        "Writes a cell.",
    ]
    for expression, token, name in [
        ("++cell[true]", "++", "inc"),
        ("cell[false]--", "--", "dec"),
        ("cell[true] += value", "+=", "plus"),
    ]:
        assert query(expression, "[", "definition") == [get, set_]
        assert query(expression, "[", "hover")["contents"] == [
            get_signature,
            "Reads a cell.",
            set_signature,
            "Writes a cell.",
        ]
        assert query(expression, token, "definition") == target(
            provider, library, f"fun {name}(", name
        )
    all_subscripts = refs("++cell[true]", "[")
    assert locations(all_subscripts) == locations(refs("/* 🌟 */ cell[true]", "[")) | locations(
        refs("cell[false] = value", "[")
    )
    assert len(all_subscripts) == len(locations(all_subscripts))
    assert query("readonly[true] += amount", "+=", "definition") == target(
        provider, library, "fun plusAssign(", "plusAssign"
    )
    assert query("readonly[true] += amount", "[", "definition") == target(
        provider, library, "get(index: Boolean): SubscriptAmount", "get"
    )

    alias_get = target(provider, library, "SubscriptAliasCell.get", "get")
    alias_set = target(provider, library, "SubscriptAliasCell.set", "set")
    assert query("alias[true]++", "[", "definition") == [alias_get, alias_set]
    assert query("alias[true]++", "[", "hover")["contents"] == [
        {
            "language": "scoop",
            "value": "fun SubscriptAliasCell.get(index: Boolean): SubscriptStep<Boolean>",
        },
        "Reads an alias cell.",
        {
            "language": "scoop",
            "value": "fun SubscriptAliasCell.set(index: Boolean, value: SubscriptStep<Boolean>): Unit",
        },
        "Writes an alias cell.",
    ]
    assert len(refs("readSubscript", "readSubscript")) == 2
    assert len(refs("writeSubscript", "writeSubscript")) == 2
    assert locations(refs("alias[true]++", "[")) == locations(
        [
            target(path, source, "readSubscript", "readSubscript"),
            target(path, source, "writeSubscript", "writeSubscript"),
            target(path, source, "alias[true]++", "["),
        ]
    )

    change(peer, provider.as_uri(), 2, library.replace("Reads a cell.", "Reads the updated cell."))
    assert query("++cell[true]", "[", "hover")["contents"] == [
        get_signature,
        "Reads the updated cell.",
        set_signature,
        "Writes a cell.",
    ]
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    assert query("++cell[true]", "[", "hover")["contents"] == [
        get_signature,
        "Reads a cell.",
        set_signature,
        "Writes a cell.",
    ]
    original = source
    source = source.replace("cell[true] += value", "val fallbackRemoved = value")
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(refs("++cell[true]", "[")) == len(all_subscripts) - 1
    source += (
        "fun subscriptBroken(box: SubscriptCell<SubscriptStep<Boolean>>) { box[true] += false }\n"
    )
    change(peer, path.as_uri(), 3, source)
    assert published(peer, path.as_uri(), 3)
    assert query("box[true] += false", "[", "hover") is None
    assert query("box[true] += false", "+=", "definition") is None
    assert len(refs("++cell[true]", "[")) == len(all_subscripts) - 1
    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(refs("++cell[true]", "[")) == len(all_subscripts)
