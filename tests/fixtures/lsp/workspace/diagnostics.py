"""Semantic diagnostics use real frontend inputs and current document versions."""


def change(peer, uri, version, text):
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}],
        },
    )


def published(peer, uri, version):
    # The worker handles the preceding diagnostic work before this request.
    peer.request("textDocument/documentSymbol", {"textDocument": {"uri": uri}})
    result = peer.diagnostics[uri]
    assert result["version"] == version, result
    return result["diagnostics"]


def check(peer, root):
    path = root / "src/diagnostics.scoop"
    path.write_text("fun checked(): Boolean = true\n")
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": uri,
                "version": 1,
                "languageId": "scoop",
                "text": (
                    "fun brokenType(value: Missing): Boolean = true\n"
                    'fun brokenBody(): Boolean = "wrong"\n'
                    "fun damaged() { true. }\n"
                    "fun healthy(): Boolean = true\n"
                ),
            }
        },
    )
    errors = published(peer, uri, 1)
    assert any(
        item["range"]["start"] == {"line": 0, "character": 22} and "Missing" in item["message"]
        for item in errors
    ), errors
    assert any(
        item["range"]["start"]["line"] == 1
        and item["severity"] == 1
        and "Boolean" in item["message"]
        for item in errors
    ), errors
    assert any(item["range"]["start"]["line"] == 2 for item in errors), errors
    assert all(item["range"]["start"]["line"] != 3 for item in errors), errors
    # Repeated requests consume the same checked body facts.
    assert published(peer, uri, 1) == errors

    change(peer, uri, 2, "fun checked(): Boolean = helper()\n")
    assert published(peer, uri, 2) == []

    helper = (root / "src/helper.scoop").as_uri()
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": helper,
                "version": 1,
                "languageId": "scoop",
                "text": "// 中文😀\nfun helper(value: Missing = true): Boolean = true\n",
            }
        },
    )
    errors = published(peer, uri, 2)
    error = next(item for item in errors if "helper" in item["message"])
    assert error["range"]["start"] == {"line": 0, "character": 25}, error
    notes = error["relatedInformation"]
    assert len(notes) == 1, notes
    assert notes[0]["location"]["uri"] == helper, notes
    assert notes[0]["location"]["range"] == {
        "start": {"line": 1, "character": 4},
        "end": {"line": 1, "character": 10},
    }, notes
    assert any("Missing" in item["message"] for item in published(peer, helper, 1))

    # Changing another open file invalidates the unchanged caller's errors.
    change(peer, helper, 2, "fun helper(value: Boolean = true): Boolean = value\n")
    assert published(peer, uri, 2) == []
    assert published(peer, helper, 2) == []
    peer.notify("textDocument/didClose", {"textDocument": {"uri": helper}})
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
