"""M60 comments retain their bound source and version in real hover requests."""

from installed_sources import boolean_location


def check(peer, root, sysroot):
    provider = root / "dependency/src/documented.scoop"
    library = (
        "package manuals\n"
        "/// **原始文档** returns [input] through [target]. [Boolean] is builtin.\n"
        "/// @param input The supplied flag.\n"
        "public fun documented(input: Boolean): Boolean = target(input)\n"
        "/// The target function.\n"
        "public fun target(input: Boolean): Boolean = input\n"
        "/// The documented property.\n"
        "public val flag: Boolean = true\n"
    )
    provider.write_text(library)
    path = root / "src/documentation.scoop"
    source = (
        "import manuals.documented\nimport manuals.flag\n"
        "/// Local **documentation** and [input].\n"
        "/// @param input The local flag.\n"
        "fun localDoc(input: Boolean): Boolean = input\n"
        "fun documentedCall(): Boolean = documented(input = true)\n"
        "fun documentedProperty(): Boolean = flag\n"
        "fun localCall(): Boolean = localDoc(input = true)\n"
        "fun brokenDocumentation(value: ) {}\n"
        "/// @param stored The constructor input.\n"
        "class DocumentedCell(\n"
        "/// The member property.\n"
        "val stored: Boolean)\n"
    )
    path.write_text(source)
    for file, text in [(provider, library), (path, source)]:
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

    def hover(line, name):
        return peer.request(
            "textDocument/hover",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": source.splitlines()[line].rindex(name)},
            },
        )

    for line, name in [(4, "localDoc"), (7, "localDoc")]:
        result = hover(line, name)
        signature, docs = result["contents"]
        assert signature["value"] == "fun localDoc(input: Boolean): Boolean", result
        assert "Local **documentation**" in docs, result
        assert f"[input](<{path.as_uri()}#L5>)" in docs, result
        assert "**@param** input — The local flag." in docs, result
    assert hover(4, "input")["contents"][1] == "The local flag."
    assert hover(7, "input")["contents"][1] == "The local flag."
    damaged = source.replace(
        "fun localDoc(input: Boolean): Boolean = input",
        "fun localDoc(input: Boolean): Boolean { input. }",
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 3},
            "contentChanges": [{"text": damaged}],
        },
    )
    for line in [4, 7]:
        assert "Local **documentation**" in hover(line, "localDoc")["contents"][1]
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 9},
            "contentChanges": [{"text": source}],
        },
    )

    def imported(expected):
        result = hover(5, "documented")
        signature, docs = result["contents"]
        assert signature["value"] == "fun documented(input: Boolean): Boolean", result
        assert f"**{expected}**" in docs, result
        assert f"[target](<{provider.as_uri()}#L6>)" in docs, result
        assert f"[input](<{provider.as_uri()}#L4>)" in docs, result
        boolean = boolean_location(sysroot)
        line = boolean["range"]["start"]["line"] + 1
        assert f"[Boolean](<{boolean['uri']}#L{line}>) is builtin." in docs, result
        return docs

    imported("原始文档")
    signature, docs = hover(5, "input")["contents"]
    assert signature["value"] == "input: Boolean", signature
    assert docs == "The supplied flag.", docs
    assert hover(1, "flag")["contents"][1] == "The documented property."
    assert hover(6, "flag")["contents"][1] == "The documented property."
    signature, docs = hover(12, "stored")["contents"]
    assert signature["value"] == "val stored: Boolean", signature
    assert docs == "The member property.", docs
    updated = (
        library.replace("原始文档", "更新文档")
        .replace("The supplied flag.", "The updated parameter.")
        .replace("The documented property.", "The updated property.")
        .replace("flag: Boolean = true", 'flag: String = "text"')
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 7},
            "contentChanges": [{"text": updated}],
        },
    )
    imported("更新文档")
    assert hover(5, "input")["contents"][1] == "The updated parameter."
    signature, docs = hover(1, "flag")["contents"]
    assert signature["value"] == "val flag: String", signature
    assert docs == "The updated property.", docs
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    imported("原始文档")
    assert hover(5, "input")["contents"][1] == "The supplied flag."
    signature, docs = hover(1, "flag")["contents"]
    assert signature["value"] == "val flag: Boolean", signature
    assert docs == "The documented property.", docs
    assert not (provider.parent.parent / "target").exists()
