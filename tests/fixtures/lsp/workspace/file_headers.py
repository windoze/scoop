"""Unavailable file headers preserve independent queries and original locations."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "src/headerApi.scoop"
    broken = root / "src/headerBroken.scoop"
    consumer = root / "src/headerConsumer.scoop"
    provider_source = (
        "package headerApi\n"
        "fun headerHealthy(value: String): String = value\n"
        "fun headerPick(): Boolean = true\n"
    )
    disk_source = "package headerOther\nfun headerPick(): Boolean = false\n"
    consumer_source = (
        "import headerApi.headerHealthy\n"
        "import headerApi.headerPick\n"
        "fun headerUser(参数: String): String = headerHealthy(参数)\n"
        "fun headerReceiver(参数: String) { 参数. }\n"
        "fun headerBlocked(): Boolean = headerPick()\n"
    )
    for path, source in [
        (provider, provider_source),
        (broken, disk_source),
        (consumer, consumer_source),
    ]:
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

    def query(method, line, name, **extra):
        return peer.request(
            method,
            {
                "textDocument": {"uri": consumer.as_uri()},
                "position": {
                    "line": line,
                    "character": consumer_source.splitlines()[line].rindex(name),
                },
                **extra,
            },
        )

    def independent():
        for line in [2, 3]:
            hover = query("textDocument/hover", line, "参数")
            assert hover["contents"]["value"] == "参数: String", hover
            definition = query("textDocument/definition", line, "参数")
            assert definition["uri"] == consumer.as_uri(), definition
            assert definition["range"]["start"] == {
                "line": line,
                "character": consumer_source.splitlines()[line].index("参数:"),
            }, definition
        definition = query("textDocument/definition", 2, "headerHealthy")
        assert definition["uri"] == provider.as_uri(), definition
        assert definition["range"]["start"] == {"line": 1, "character": 4}, definition
        references = query(
            "textDocument/references",
            2,
            "headerHealthy",
            context={"includeDeclaration": True},
        )
        assert len(references) == 3, references
        assert {item["uri"] for item in references} == {
            consumer.as_uri(),
            provider.as_uri(),
        }, references

    independent()
    assert query("textDocument/definition", 4, "headerPick")["uri"] == provider.as_uri()
    for version, source in [
        (2, "package\nfun headerPick(): Boolean = false\n"),
        (3, "package headerApi\nimport headerApi.\nfun headerPick(): Boolean = false\n"),
    ]:
        change(peer, broken.as_uri(), version, source)
        errors = published(peer, broken.as_uri(), version)
        assert any("package or imports" in error["message"] for error in errors), errors
        independent()
        assert query("textDocument/definition", 4, "headerPick") is None
        errors = published(peer, consumer.as_uri(), 1)
        assert any(error["range"]["start"]["line"] == 4 for error in errors), errors
        assert all(error["range"]["start"]["line"] not in [0, 1, 2] for error in errors), errors
    change(peer, broken.as_uri(), 4, disk_source)
    independent()
    definition = query("textDocument/definition", 4, "headerPick")
    assert definition["uri"] == provider.as_uri(), definition
    assert definition["range"]["start"] == {"line": 2, "character": 4}, definition
    assert published(peer, broken.as_uri(), 4) == []
    change(peer, broken.as_uri(), 5, "package\nfun headerPick(): Boolean = false\n")
    independent()
    assert query("textDocument/definition", 4, "headerPick") is None
    peer.notify("textDocument/didClose", {"textDocument": {"uri": broken.as_uri()}})
    independent()
    assert query("textDocument/definition", 4, "headerPick")["uri"] == provider.as_uri()
    assert not (root / "target").exists()
