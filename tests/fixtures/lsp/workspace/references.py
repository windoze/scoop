"""Real bound references across declarations, scopes, files and input versions."""


def check(peer, root):
    path = root / "src/references.scoop"
    text = (
        "fun refChoose(value: Boolean): Boolean = value\n"
        "fun refChoose(value: String): String = value\n"
        "fun <Item> refSame(value: Item): Item = value\n"
        "fun refOwner(input: Boolean): Boolean {\n"
        "    val copy = input\n"
        "    if (input) { val copy = false; val inner = copy }\n"
        "    val captured: () -> Boolean = { copy }\n"
        "    val result = refChoose(captured())\n"
        '    val second = refChoose("text")\n'
        '    val third = refSame("text")\n'
        "    return refSame(result)\n"
        "}\n"
    )
    caller = root / "src/referenceCaller.scoop"
    call_text = "fun refCaller(): Boolean = refOwner(input = true)\n"
    for file, contents in [(path, text), (caller, call_text)]:
        file.write_text(contents)
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": file.as_uri(),
                    "languageId": "scoop",
                    "version": 1,
                    "text": contents,
                }
            },
        )

    def references(line, name, include=True):
        return peer.request(
            "textDocument/references",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": text.splitlines()[line].index(name)},
                "context": {"includeDeclaration": include},
            },
        )

    for line, name, expected in [
        (0, "refChoose", [0, 7]),
        (1, "refChoose", [1, 8]),
        (2, "refSame", [2, 9, 10]),
        (4, "copy", [4, 6]),
        (5, "copy", [5, 5]),
    ]:
        result = references(line, name)
        assert all(location["uri"] == path.as_uri() for location in result), result
        assert [location["range"]["start"]["line"] for location in result] == expected, result
    uses = references(0, "refChoose", False)
    assert len(uses) == 1 and uses[0]["range"]["start"]["line"] == 7, uses
    parameter = references(3, "input")
    assert len(parameter) == 4, parameter
    assert sum(location["uri"] == caller.as_uri() for location in parameter) == 1, parameter
    named = peer.request(
        "textDocument/references",
        {
            "textDocument": {"uri": caller.as_uri()},
            "position": {"line": 0, "character": call_text.index("input")},
            "context": {"includeDeclaration": True},
        },
    )
    assert named == parameter, (named, parameter)
    definition = peer.request(
        "textDocument/definition",
        {
            "textDocument": {"uri": caller.as_uri()},
            "position": {"line": 0, "character": call_text.index("input")},
        },
    )
    assert definition["uri"] == path.as_uri(), definition
    assert definition["range"]["start"] == {
        "line": 3,
        "character": text.splitlines()[3].index("input"),
    }, definition
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": caller.as_uri(), "version": 5},
            "contentChanges": [{"text": "fun refCaller(): Boolean = true\n"}],
        },
    )
    assert len(references(3, "input")) == 3
    assert references(3, "refOwner", False) == []
    peer.notify("textDocument/didClose", {"textDocument": {"uri": caller.as_uri()}})
    assert len(references(3, "input")) == 4
    assert not peer.diagnostics[path.as_uri()]["diagnostics"], peer.diagnostics[path.as_uri()]
