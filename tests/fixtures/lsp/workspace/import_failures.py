"""Semantic import failures keep independent declarations and lookup barriers."""

from diagnostics import change, published


def check(peer, root):
    project = root / "import-failures"
    (project / "src").mkdir(parents=True, exist_ok=True)
    (project / "Cone.toml").write_text(
        'schema = 1\n[cone]\ngroup = "test"\nname = "import-failures"\n'
        'version = "1.0.0"\nkind = "library"\n'
    )
    api = project / "src/api.scoop"
    use = project / "src/use.scoop"
    other = project / "src/other.scoop"
    original = (
        "import missing.pick as selected\n"
        "import failureapi.good as imported\n"
        "fun independent(value: Boolean): Boolean = imported(value)\n"
        "fun broken(): Boolean = selected(true)\n"
        "fun shadowed(selected: Boolean): Boolean = selected\n"
    )
    texts = {
        api: "package failureapi\nfun good(value: Boolean): Boolean = value\n",
        use: original,
        other: "typealias Flag = Boolean\nfun selected(value: Boolean): Boolean = value\n",
    }
    for file, text in texts.items():
        file.write_text(text)
    folder = {"name": "import failures", "uri": project.as_uri()}
    peer.notify(
        "workspace/didChangeWorkspaceFolders", {"event": {"added": [folder], "removed": []}}
    )
    for file, text in texts.items():
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

    def query(file, needle, name, method, error=None, **extra):
        text = texts[file]
        prefix = text[: text.index(needle) + needle.index(name)]
        request = peer.send_request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": file.as_uri()},
                "position": {
                    "line": prefix.count("\n"),
                    "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
                },
                **extra,
            },
        )
        return peer.response(request, error_code=error)

    def references(file, needle, name, count):
        result = query(file, needle, name, "references", context={"includeDeclaration": True})
        assert result is not None and len(result) == count, result

    assert published(peer, api.as_uri(), 1) == []
    assert published(peer, other.as_uri(), 1) == []
    diagnostics = published(peer, use.as_uri(), 1)
    assert any("import binding for `selected`" in item["message"] for item in diagnostics), (
        diagnostics
    )
    assert query(use, "fun independent", "independent", "hover") is not None
    result = query(use, "imported(value)", "imported", "definition")
    assert result["uri"] == api.as_uri(), result
    assert query(use, "selected(true)", "selected", "definition") is None
    assert query(use, "= selected\n", "selected", "hover") is not None
    references(api, "fun good", "good", 3)
    references(other, "fun selected", "selected", 1)
    query(use, "fun independent", "independent", "prepareRename", error=-32803)

    texts[use] = original.replace("missing.pick", "failureapi.good")
    change(peer, use.as_uri(), 2, texts[use])
    assert published(peer, use.as_uri(), 2) == []
    result = query(use, "selected(true)", "selected", "definition")
    assert result["uri"] == api.as_uri(), result
    references(api, "fun good", "good", 5)

    texts[use] = (
        "import missing.*\nimport failureapi.good as imported\n"
        "fun independent(value: Flag): Flag = value\n"
        "fun exact(): Flag = imported(true)\n"
        "fun current(): Flag = selected(true)\n"
        "fun prelude(value: String): Flag = true\n"
    )
    change(peer, use.as_uri(), 3, texts[use])
    diagnostics = published(peer, use.as_uri(), 3)
    assert any("import binding for `String`" in item["message"] for item in diagnostics), (
        diagnostics
    )
    result = query(use, "imported(true)", "imported", "definition")
    assert result["uri"] == api.as_uri(), result
    result = query(use, "selected(true)", "selected", "definition")
    assert result["uri"] == other.as_uri(), result
    assert query(use, "fun independent", "independent", "hover") is not None

    peer.notify("textDocument/didClose", {"textDocument": {"uri": use.as_uri()}})
    texts[use] = original
    assert query(use, "selected(true)", "selected", "definition") is None
    assert query(use, "imported(value)", "imported", "definition")["uri"] == api.as_uri()
    references(api, "fun good", "good", 3)
    for file in [api, other]:
        peer.notify("textDocument/didClose", {"textDocument": {"uri": file.as_uri()}})
    peer.notify(
        "workspace/didChangeWorkspaceFolders", {"event": {"added": [], "removed": [folder]}}
    )
