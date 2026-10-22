"""Source libraries join the same semantic world as precompiled dependencies."""


def check(peer, root):
    dependency = root / "dependency"
    (dependency / "src").mkdir(parents=True)
    (dependency / "Cone.toml").write_text(
        'schema = 1\n[cone]\ngroup = "test"\nname = "editor-source"\n'
        'version = "1.0.0"\nkind = "library"\n'
    )
    path = dependency / "src/library.scoop"
    original = (
        "package shared\n"
        "public typealias Flag = Boolean\n"
        "public fun <Item> pass(value: Item): Item = value\n"
        "public fun choose(value: Flag = true): Boolean = pass(value)\n"
        "public fun answer(): Boolean = choose()\n"
    )
    path.write_text(original)
    manifest = (root / "Cone.toml").as_uri()
    text = (
        (root / "Cone.toml")
        .read_text()
        .replace(
            "[dependencies]",
            '[dependencies]\n"test:editor-source" = {version = "1.0.0", path = "dependency"}',
        )
    )
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": manifest, "version": 3}, "contentChanges": [{"text": text}]},
    )
    consumer = root / "src/sourceConsumer.scoop"
    consumer_text = (
        "import shared.answer\nimport shared.pass as renamed\nimport shared.choose\n"
        "fun call(): Boolean = renamed(choose())\n"
        "fun editing() { val value = answer(); value. }\n"
    )
    consumer.write_text(consumer_text)
    uri = consumer.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": consumer_text}},
    )
    line = consumer_text.splitlines()[4]
    cursor = line.rindex("value.")

    def hover():
        return peer.request(
            "textDocument/hover",
            {"textDocument": {"uri": uri}, "position": {"line": 4, "character": cursor}},
        )

    def value_type(expected):
        result = hover()
        assert result["contents"]["value"] == f"value: {expected}", result
        result = peer.request(
            "textDocument/completion",
            {"textDocument": {"uri": uri}, "position": {"line": 4, "character": cursor + 6}},
        )
        assert any(item["label"] == "toString" for item in result["items"]), result

    value_type("Boolean")
    call = peer.request(
        "textDocument/hover",
        {"textDocument": {"uri": uri}, "position": {"line": 3, "character": 4}},
    )
    assert call["contents"]["value"] == "fun call(): Boolean", call
    call_line = consumer_text.splitlines()[3]
    for name, expected, target_line in [
        ("renamed", "fun <Item> pass(value: Item): Item", 2),
        ("choose", "fun choose(value: Boolean = true): Boolean", 3),
    ]:
        position = {"line": 3, "character": call_line.index(name)}
        params = {"textDocument": {"uri": uri}, "position": position}
        result = peer.request("textDocument/hover", params)
        assert result["contents"]["value"] == expected, result
        destination = peer.request("textDocument/definition", params)
        assert destination["uri"] == path.as_uri(), destination
        assert destination["range"]["start"]["line"] == target_line, destination
    assert not peer.diagnostics[manifest]["diagnostics"], peer.diagnostics[manifest]
    assert not any(
        "not found" in error["message"] for error in peer.diagnostics[uri]["diagnostics"]
    )

    dependency_uri = path.as_uri()
    changed = original.replace("answer(): Boolean = choose()", 'answer(): String = "changed"')
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": dependency_uri,
                "languageId": "scoop",
                "version": 1,
                "text": changed,
            }
        },
    )
    value_type("String")
    broken = changed.replace('= "changed"', "= missing")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": dependency_uri, "version": 2},
            "contentChanges": [{"text": broken}],
        },
    )
    assert hover() is None, "a broken source library must not reuse its previous interface"
    symbols = peer.request("textDocument/documentSymbol", {"textDocument": {"uri": uri}})
    assert [item["name"] for item in symbols] == ["call", "editing"], symbols
    assert any(
        "missing" in error["message"] for error in peer.diagnostics[dependency_uri]["diagnostics"]
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": dependency_uri, "version": 3},
            "contentChanges": [{"text": changed}],
        },
    )
    value_type("String")
    assert not peer.diagnostics[dependency_uri]["diagnostics"], peer.diagnostics[dependency_uri]
    peer.notify("textDocument/didClose", {"textDocument": {"uri": dependency_uri}})
    value_type("Boolean")
    assert not (dependency / "target").exists()
