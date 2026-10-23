"""Explicit accessors share body queries without exposing implementation functions."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/accessors.scoop"
    library = (
        "package accessorapi\n"
        "public class Payload public constructor(public val flag: Boolean)\n"
        "/// @param value The imported flag.\n"
        "public fun pass(value: Boolean): Boolean = value\n"
    )
    path = root / "src/accessors.scoop"
    source = (
        "import accessorapi.Payload\n"
        "import accessorapi.pass as accessorPass\n"
        "var accessorFlag: Boolean = true\n"
        "get() { val savedFlag = field; return accessorPass(value = savedFlag) }\n"
        "/// @param replacementFlag The replacement flag.\n"
        "set(replacementFlag) { field = accessorPass(value = replacementFlag) }\n"
        "class AccessorLocalBox<T>(var item: T) {\n"
        "    var projected: T\n"
        "    get() { val copiedItem = item; return copiedItem }\n"
        "    set(nextItem) { item = nextItem }\n"
        "}\n"
        "var accessorPayload: Payload = Payload(true)\n"
        "get() = field\n"
        "/// @param replacementPayload The payload replacement.\n"
        "set(replacementPayload) { val localPayload = replacementPayload; field = localPayload }\n"
        "val accessorMember: Boolean get() = accessorPayload.flag\n"
        "val accessorNested: Boolean get() {\n"
        "    class AccessorNestedValue(val value: Boolean) {\n"
        "        val read: Boolean get() { val nestedCopy = value; return nestedCopy }\n"
        "    }\n"
        "    return AccessorNestedValue(true).read\n"
        "}\n"
        "var accessorImplicit: Boolean = true\n"
        "get() = field\n"
        "set() { field = value }\n"
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
        ("value = savedFlag", "savedFlag", "savedFlag =", "savedFlag: Boolean"),
        ("return copiedItem", "copiedItem", "copiedItem =", "copiedItem: T"),
        ("item = nextItem", "nextItem", "nextItem)", "nextItem: T"),
        ("return nestedCopy", "nestedCopy", "nestedCopy =", "nestedCopy: Boolean"),
    ]:
        hover = query(needle, name, "hover")
        assert hover["contents"]["value"] == expected, hover
        definition = query(needle, name, "definition")
        assert definition == target(path, source, declaration, name), definition
        references = query(needle, name, "references", context={"includeDeclaration": True})
        assert len(references) == 2 and definition in references, references

    for needle, name, expected, doc in [
        ("set(replacementFlag)", "replacementFlag", "Boolean", "The replacement flag."),
        ("value = replacementFlag", "replacementFlag", "Boolean", "The replacement flag."),
        ("set(replacementPayload)", "replacementPayload", "Payload", "The payload replacement."),
        (
            "localPayload = replacementPayload",
            "replacementPayload",
            "Payload",
            "The payload replacement.",
        ),
    ]:
        hover = query(needle, name, "hover")
        signature, description = hover["contents"]
        assert signature["value"] == f"{name}: {expected}" and description == doc, hover
        references = query(needle, name, "references", context={"includeDeclaration": True})
        assert len(references) == 2, references

    imported = target(provider, library, "fun pass", "pass")
    assert query("accessorPass(value = savedFlag)", "accessorPass", "definition") == imported
    call = query("accessorPass(value = savedFlag)", "savedFlag", "signatureHelp")
    assert call["signatures"][0]["label"] == "accessorPass(value: Boolean): Boolean", call
    argument = query("accessorPass(value = savedFlag)", "value", "hover")
    assert argument["contents"][1] == "The imported flag.", argument
    replacement = target(provider, library, "class Payload", "Payload")
    assert query("set(replacementPayload)", "replacementPayload", "typeDefinition") == replacement
    assert (
        query("localPayload = replacementPayload", "replacementPayload", "typeDefinition")
        == replacement
    )
    members = query("accessorPayload.flag", "flag", "completion", offset=2)
    assert any(item["label"] == "flag" for item in members["items"]), members
    implicit = query("field = value", "value", "hover")
    assert implicit["contents"]["value"] == "value: Boolean", implicit
    implicit_origin = query("field = value", "value", "definition")
    slot = position(source, "set()", ")")
    assert implicit_origin == {"uri": path.as_uri(), "range": {"start": slot, "end": slot}}, (
        implicit_origin
    )

    previous = source
    source = source.replace(
        "return accessorPass(value = savedFlag)",
        "val completedFlag = accessorPass(value = savedFlag); return missingAccessorValue",
    )
    change(peer, path.as_uri(), 2, source)
    errors = published(peer, path.as_uri(), 2)
    assert any("missingAccessorValue" in error["message"] for error in errors), errors
    assert all(error["range"]["start"]["line"] == 3 for error in errors), errors
    assert query("completedFlag =", "completedFlag", "hover") is None
    call = query("accessorPass(value = savedFlag)", "savedFlag", "signatureHelp")
    assert call["signatures"][0]["label"] == "accessorPass(value: Boolean): Boolean", call
    setter = query("value = replacementFlag", "replacementFlag", "hover")
    assert setter["contents"][1] == "The replacement flag.", setter
    source = previous
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    restored = query("value = savedFlag", "savedFlag", "hover")
    assert restored["contents"]["value"] == "savedFlag: Boolean", restored

    source = source.replace("The replacement flag.", "Updated replacement flag.")
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": path.as_uri(),
                "languageId": "scoop",
                "version": 4,
                "text": source,
            }
        },
    )
    updated = query("value = replacementFlag", "replacementFlag", "hover")
    assert updated["contents"][1] == "Updated replacement flag.", updated
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
