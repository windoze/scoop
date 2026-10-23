"""Runtime initializer source regions participate in normal semantic queries."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/initializers.scoop"
    library = (
        "package initializerapi\n"
        "public class Payload public constructor(public val flag: Boolean)\n"
        "/// @param value The initial input.\n"
        "public fun choose(value: Boolean): Boolean = value\n"
    )
    path = root / "src/initializers.scoop"
    source = (
        "import initializerapi.choose as initializerChoose\n"
        "import initializerapi.Payload as InitializerPayload\n"
        "val initializerSeed: Boolean = true\n"
        "val initializerResult = initializerChoose(value = initializerSeed)\n"
        "val initializerCapture: Boolean = if (initializerSeed) {\n"
        "    val captured = initializerResult\n"
        "    val read: () -> Boolean = { captured }\n"
        "    fun <T> keep(value: T): T = value\n"
        "    keep(read())\n"
        "} else false\n"
        "val initializerObject: InitializerPayload = InitializerPayload(flag = initializerResult)\n"
        "val initializerMember: Boolean = initializerObject.flag\n"
        "val initializerNested: Boolean = if (true) {\n"
        "    class InitializerInner(val item: Boolean) {\n"
        "        val copied: Boolean get() { val saved = item; return saved }\n"
        "    }\n"
        "    InitializerInner(true).copied\n"
        "} else false\n"
        "class InitializerBox<T>(val item: T)\n"
        "class InitializerSlot<T> {\n"
        "    operator fun getValue(thisRef: InitializerBox<T>): T = thisRef.item\n"
        "}\n"
        "val <T> InitializerBox<T>.initializerDelegated: T by if (true) {\n"
        "    fun retain(value: T): T = value\n"
        "    val read: (T) -> T = ::retain\n"
        "    InitializerSlot<T>()\n"
        "} else InitializerSlot<T>()\n"
        "val initializerCandidate: Boolean = initializerChoose(value = true)\n"
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

    def position(text, needle, name, offset=0):
        prefix = text[: text.index(needle) + needle.index(name) + offset]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, name, method, offset=0, **extra):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": position(source, needle, name, offset),
                **extra,
            },
        )

    def target(file, text, needle, name):
        start = position(text, needle, name)
        end = {**start, "character": start["character"] + len(name)}
        return {"uri": file.as_uri(), "range": {"start": start, "end": end}}

    assert published(peer, path.as_uri(), 1) == []
    for needle, name, declaration, expected in [
        ("{ captured }", "captured", "captured =", "captured: Boolean"),
        ("return saved", "saved", "saved =", "saved: Boolean"),
        ("T = value\n    val read", "value", "retain(value:", "value: T"),
    ]:
        hover = query(needle, name, "hover")
        assert hover["contents"]["value"] == expected, hover
        definition = query(needle, name, "definition")
        assert definition == target(path, source, declaration, name), definition
        references = query(needle, name, "references", context={"includeDeclaration": True})
        assert len(references) == 2 and definition in references, references

    seed = query("value = initializerSeed", "initializerSeed", "hover")
    assert seed["contents"]["value"] == "val initializerSeed: Boolean", seed
    refs = query(
        "value = initializerSeed",
        "initializerSeed",
        "references",
        context={"includeDeclaration": True},
    )
    assert len(refs) == 3, refs
    inferred = query("initializerResult =", "initializerResult", "hover")
    assert inferred["contents"]["value"] == "val initializerResult: Boolean", inferred
    argument = query("initializerChoose(value = initializerSeed)", "value", "hover")
    assert argument["contents"][1] == "The initial input.", argument
    imported = target(provider, library, "fun choose", "choose")
    assert (
        query("initializerChoose(value = initializerSeed)", "initializerChoose", "definition")
        == imported
    )
    call = query("initializerChoose(value = initializerSeed)", "initializerSeed", "signatureHelp")
    assert call["signatures"][0]["label"] == "initializerChoose(value: Boolean): Boolean", call
    payload = target(provider, library, "class Payload", "Payload")
    assert query("initializerObject.flag", "initializerObject", "typeDefinition") == payload
    members = query("initializerObject.flag", "flag", "completion", offset=2)
    assert any(item["label"] == "flag" for item in members["items"]), members

    def alias_references():
        return query(
            "initializerChoose(value = initializerSeed)",
            "initializerChoose",
            "references",
            context={"includeDeclaration": True},
        )

    assert len(alias_references()) == 3
    original = source
    source = source.replace(
        "initializerCandidate: Boolean = initializerChoose(value = true)",
        "initializerCandidate: Boolean = true",
    )
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2) == []
    assert len(alias_references()) == 2

    source = original.replace(
        "initializerCandidate: Boolean = initializerChoose(value = true)",
        "initializerCandidate: Boolean = if (true) { val completed = initializerChoose(value = true); missingInitializerValue } else false",
    )
    change(peer, path.as_uri(), 3, source)
    errors = published(peer, path.as_uri(), 3)
    assert any("missingInitializerValue" in error["message"] for error in errors), errors
    assert query("completed =", "completed", "hover") is None
    call = query("initializerChoose(value = true)", "true", "signatureHelp")
    assert call["signatures"][0]["label"] == "initializerChoose(value: Boolean): Boolean", call
    assert len(alias_references()) == 2
    assert query("{ captured }", "captured", "hover")["contents"]["value"] == "captured: Boolean"

    source = original
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert len(alias_references()) == 3
    assert (
        query("initializerResult =", "initializerResult", "hover")["contents"]["value"]
        == "val initializerResult: Boolean"
    )
