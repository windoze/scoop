"""Failed source types cannot expose partial signatures or erase independent facts."""

from diagnostics import change, published


def check(peer, root):
    project = root / "nominal-failures"
    (project / "src").mkdir(parents=True, exist_ok=True)
    (project / "Cone.toml").write_text(
        'schema = 1\n[cone]\ngroup = "test"\nname = "nominal-failures"\n'
        'version = "1.0.0"\nkind = "library"\n'
    )
    source = project / "src/types.scoop"
    consumer = project / "src/use.scoop"
    healthy = (
        "fun independent(标志: Boolean): Boolean = 标志\n"
        "interface Contract {}\nclass Implementation : Contract {}\n"
    )
    original = (
        "// 声明失败 😀\nstruct Broken(val bad: Missing, val kept: Boolean)\n"
        + healthy
        + "fun construct(): Boolean { val item = Broken(true); return item.kept }\n"
    )
    texts = {
        source: original,
        consumer: "fun consume(): Boolean = independent(true)\n",
    }
    for file, text in texts.items():
        file.write_text(text)
    folder = {"name": "nominal failures", "uri": project.as_uri()}
    peer.resolve(project)
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

    def independent():
        assert query(source, "fun independent", "independent", "hover") is not None
        result = query(consumer, "independent(true)", "independent", "definition")
        assert result["uri"] == source.as_uri(), result
        references = query(
            source,
            "fun independent",
            "independent",
            "references",
            context={"includeDeclaration": True},
        )
        assert references is not None and len(references) == 2, references
        implementations = query(source, "interface Contract", "Contract", "implementation")
        assert implementations is not None and len(implementations) == 1, implementations
        assert published(peer, consumer.as_uri(), 1) == []

    diagnostics = published(peer, source.as_uri(), 1)
    assert any("Missing" in item["message"] for item in diagnostics), diagnostics
    assert query(source, "struct Broken", "Broken", "hover") is None
    assert query(source, "Broken(true)", "Broken", "hover") is None
    assert query(source, "item.kept", "kept", "definition") is None
    independent()
    query(source, "fun independent", "independent", "prepareRename", error=-32803)

    texts[source] = original.replace("val bad: Missing, ", "")
    change(peer, source.as_uri(), 2, texts[source])
    assert published(peer, source.as_uri(), 2) == []
    hover = query(source, "Broken(true)", "Broken", "hover")
    assert hover is not None and "kept: Boolean" in hover["contents"]["value"], hover
    assert query(source, "item.kept", "kept", "definition") is not None
    independent()

    for version, declaration in enumerate(
        [
            "interface Cycle : Cycle {}",
            "open class Cycle : Cycle() {}",
            "struct Cycle<T>(val next: Cycle<(T, T)>)",
        ],
        3,
    ):
        texts[source] = "// 循环 😀\n" + declaration + "\n" + healthy
        change(peer, source.as_uri(), version, texts[source])
        diagnostics = published(peer, source.as_uri(), version)
        assert any(
            "inherits from itself" in item["message"] or "recursive value layout" in item["message"]
            for item in diagnostics
        ), diagnostics
        assert query(source, "Cycle", "Cycle", "hover") is None
        independent()

    texts[source] = original.replace("val bad: Missing, ", "")
    change(peer, source.as_uri(), 6, texts[source])
    assert published(peer, source.as_uri(), 6) == []
    assert query(source, "Broken(true)", "Broken", "hover") is not None
    peer.notify("textDocument/didClose", {"textDocument": {"uri": source.as_uri()}})
    texts[source] = original
    assert query(source, "Broken(true)", "Broken", "hover") is None
    independent()
    peer.notify("textDocument/didClose", {"textDocument": {"uri": consumer.as_uri()}})
    peer.notify(
        "workspace/didChangeWorkspaceFolders", {"event": {"added": [], "removed": [folder]}}
    )
