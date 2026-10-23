"""Alias references keep their spelling, overload set and current source version."""


def check(peer, root):
    provider = root / "src/aliasProvider.scoop"
    provider_text = (
        "object AliasApi {\n"
        "fun choose(value: Boolean): Boolean = value\n"
        "fun choose(value: String): String = value\n"
        "}\n"
    )
    consumer = root / "src/aliasConsumer.scoop"
    consumer_text = (
        "import AliasApi.choose as 别名\n"
        "fun aliasUse(): Boolean {\n"
        "    val first = 别名(true)\n"
        '    val second = 别名("text")\n'
        "    return AliasApi.choose(first)\n"
        "}\n"
        "fun aliasShadowed(): Boolean {\n"
        "    fun 别名(value: Boolean): Boolean = value\n"
        "    return 别名(true)\n"
        "}\n"
    )
    same = root / "src/sameAlias.scoop"
    same_text = "import AliasApi.choose as choose\nfun sameAlias(): Boolean = choose(true)\n"
    for path, text in [(provider, provider_text), (consumer, consumer_text), (same, same_text)]:
        path.write_text(text)
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": path.as_uri(),
                    "languageId": "scoop",
                    "version": 1,
                    "text": text,
                }
            },
        )

    def references(path, text, line, name, include=True, last=False):
        content = text.splitlines()[line]
        character = content.rindex(name) if last else content.index(name)
        return peer.request(
            "textDocument/references",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": character},
                "context": {"includeDeclaration": include},
            },
        )

    expected = references(consumer, consumer_text, 0, "别名")
    assert [item["range"]["start"]["line"] for item in expected] == [0, 2, 3], expected
    assert all(item["uri"] == consumer.as_uri() for item in expected), expected
    assert references(consumer, consumer_text, 2, "别名") == expected
    assert len(references(consumer, consumer_text, 3, "别名", False)) == 2
    local = references(consumer, consumer_text, 7, "别名")
    assert [item["range"]["start"]["line"] for item in local] == [7, 8], local
    named = references(same, same_text, 0, "choose", last=True)
    assert len(named) == 2 and all(item["uri"] == same.as_uri() for item in named), named
    original = references(provider, provider_text, 1, "choose")
    assert len(original) == 6, original
    assert references(consumer, consumer_text, 4, "choose") == original
    overloads = references(consumer, consumer_text, 0, "choose")
    assert sum(item["uri"] == provider.as_uri() for item in overloads) == 2, overloads
    for name in ["choose", "别名"]:
        definitions = peer.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": consumer.as_uri()},
                "position": {"line": 0, "character": consumer_text.splitlines()[0].index(name)},
            },
        )
        assert len(definitions) == 2, definitions
        assert all(item["uri"] == provider.as_uri() for item in definitions), definitions
        assert sorted(item["range"]["start"]["line"] for item in definitions) == [1, 2], definitions

    changed = consumer_text.replace(
        "import AliasApi.choose as 别名", "import AliasApi.choose"
    ).replace("别名", "choose")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": consumer.as_uri(), "version": 7},
            "contentChanges": [{"text": changed}],
        },
    )
    current = references(consumer, changed, 2, "choose")
    assert any(item["uri"] == provider.as_uri() for item in current), current
    peer.notify("textDocument/didClose", {"textDocument": {"uri": consumer.as_uri()}})
    assert references(consumer, consumer_text, 2, "别名") == expected
    assert not peer.diagnostics[same.as_uri()]["diagnostics"], peer.diagnostics[same.as_uri()]
