"""Type definition uses checked values and transparent alias applications."""

from installed_sources import boolean_location


def check(peer, root, sysroot):
    provider = root / "dependency/src/typeDefinitions.scoop"
    library = (
        "package typedefs\n"
        "public class 原型 {}\n"
        "public typealias Same<T> = T\n"
        "public struct Box<T>(val value: T)\n"
        "public fun <T> make(value: T): T = value\n"
    )
    path = root / "src/typeDefinitions.scoop"
    source = (
        "import typedefs.原型\n"
        "import typedefs.Same\n"
        "import typedefs.Box\n"
        "import typedefs.make\n"
        "typealias Saved = 原型\n"
        "fun typedValue(input: Same<原型>): 原型 { val holder = Box(input); val value: Saved = make(holder.value); return value }\n"
        "fun <T> typedParameter(input: T): T = input\n"
        "fun structuralType(tuple: (原型, 原型), callback: () -> 原型): 原型 = callback()\n"
        "fun recoveredType(input: 原型) { input. }\n"
        "fun changingType(input: Same<原型>) = input\n"
        "fun builtinType(input: Boolean): Boolean = input\n"
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

    def query(line, name, last=False, method="typeDefinition"):
        text = source.splitlines()[line]
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {
                    "line": line,
                    "character": text.rindex(name) if last else text.index(name),
                },
            },
        )

    def expected(line, name):
        start = library.splitlines()[line].index(name)
        return {
            "uri": provider.as_uri(),
            "range": {
                "start": {"line": line, "character": start},
                "end": {"line": line, "character": start + len(name)},
            },
        }

    item = expected(1, "原型")
    box = expected(3, "Box")
    for line, name, last in [
        (4, "Saved", False),
        (5, "typedValue", False),
        (5, "input", False),
        (5, "Same", False),
        (5, "value", True),
        (5, "make", False),
        (7, "callback", True),
        (8, "input", True),
        (9, "input", False),
    ]:
        result = query(line, name, last)
        assert result == item, (line, name, result)
    assert query(5, "holder") == box
    assert query(5, "Same", method="definition") == expected(2, "Same")
    assert query(7, "tuple") is None
    assert query(7, "callback") is None
    assert query(10, "input") == boolean_location(sysroot)
    parameter = query(6, "input", last=True)
    assert parameter["uri"] == path.as_uri(), parameter
    assert parameter["range"] == {
        "start": {"line": 6, "character": 5},
        "end": {"line": 6, "character": 6},
    }, parameter

    updated = source.replace(
        "fun changingType(input: Same<原型>)", "fun changingType(input: Same<Box<原型>>)"
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 6},
            "contentChanges": [{"text": updated}],
        },
    )
    assert query(9, "input") == box
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert query(9, "input") == item
    assert not (provider.parent.parent / "target").exists()
