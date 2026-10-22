"""Signature help consumes real applied calls and syntax argument positions."""


def check(peer, root):
    path = root / "src/signatures.scoop"
    text = (
        "fun choose(first: Boolean, second: String): String = second\n"
        "fun choose(first: String, second: Boolean): String = first\n"
        "fun <T> gather(value: T, vararg rest: T): T = value\n"
        "fun accept(flag: Boolean = true, task: () -> Boolean): Boolean = task()\n"
        "fun checked(action: (Boolean) -> String, rest: Array<Boolean>): String {\n"
        '    choose(second = "中文😀", first = true)\n'
        "    gather(true, false, true)\n"
        "    gather(true, *rest)\n"
        "    accept() { true }\n"
        "    action(true)\n"
        "    helper()\n"
        '    return choose(first = helper(), second = "last")\n'
        "}\n"
        "fun importedCall(): Boolean = true.equals(false)\n"
        "fun importedEmpty(): String = true.toString()\n"
    )
    path.write_text(text)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "version": 1, "languageId": "scoop", "text": text}},
    )

    def help_at(line, column):
        # LSP uses UTF-16 by default, including after non-BMP source characters.
        prefix = text.splitlines()[line][:column]
        character = len(prefix.encode("utf-16-le")) // 2
        return peer.request(
            "textDocument/signatureHelp",
            {"textDocument": {"uri": uri}, "position": {"line": line, "character": character}},
        )

    named = text.splitlines()[5]
    result = help_at(5, named.index("中文"))
    assert result["signatures"][0]["label"] == "choose(first: Boolean, second: String): String", (
        result
    )
    assert result["activeParameter"] == 1, result
    assert help_at(5, named.index("first"))["activeParameter"] == 0
    assert help_at(5, named.index("choose")) is None
    assert help_at(5, len(named)) is None

    result = help_at(6, text.splitlines()[6].rindex("true"))
    assert (
        result["signatures"][0]["label"] == "gather(value: Boolean, vararg rest: Boolean): Boolean"
    ), result
    assert result["activeParameter"] == 1, result
    assert help_at(7, text.splitlines()[7].index("*"))["activeParameter"] == 1
    assert help_at(8, text.splitlines()[8].index("true"))["activeParameter"] == 1
    result = help_at(9, text.splitlines()[9].index("true"))
    assert result["signatures"][0]["label"] == "action(Boolean): String", result
    assert result["activeParameter"] == 0
    result = help_at(10, text.splitlines()[10].index(")"))
    assert result["signatures"][0]["label"] == "helper(value: Boolean): Boolean", result

    nested = text.splitlines()[11]
    result = help_at(11, nested.index("helper") + len("helper("))
    assert result["signatures"][0]["label"] == "helper(value: Boolean): Boolean", result
    result = help_at(11, nested.index("second"))
    assert result["signatures"][0]["label"] == "choose(first: Boolean, second: String): String", (
        result
    )
    assert result["activeParameter"] == 1
    imported = text.splitlines()[13]
    result = help_at(13, imported.index("false"))
    assert result["signatures"][0]["label"] == "equals(other: Boolean): Boolean", result
    assert result["activeParameter"] == 0
    result = help_at(14, text.splitlines()[14].rindex(")"))
    assert result["signatures"][0]["label"] == "toString(): String", result
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
