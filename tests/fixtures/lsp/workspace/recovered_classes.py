"""Class recovery keeps actual member signatures, scopes and source locations."""


def check(peer, root):
    path = root / "src/recoveredClasses.scoop"
    source = (
        "package recovered_classes\n"
        "class Box<T>(val stored: T) {\n"
        "private fun secret(): Boolean = true\n"
        "fun editing(value: T): T { value. }\n"
        "fun members() { this. }\n"
        "fun partial(value: T): T { healthy( }\n"
        "fun healthy(value: T): T = value\n"
        "}\n"
        "fun consumer(box: Box<Boolean>): Boolean = box.healthy(true)\n"
        "fun external(box: Box<Boolean>) { box. }\n"
        "class Outer<T> { inner class Inner { fun nested(value: T) { value. } } }\n"
    )
    path.write_text(source)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": source}},
    )

    def position(line, marker, last=False, after=False):
        text = source.splitlines()[line]
        offset = text.rindex(marker) if last else text.index(marker)
        if after:
            offset += len(marker)
        return {"line": line, "character": len(text[:offset].encode("utf-16-le")) // 2}

    def query(method, line, marker, last=False, after=False):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": uri},
                "position": position(line, marker, last, after),
            },
        )

    def expected(line, marker):
        return {
            "uri": uri,
            "range": {
                "start": position(line, marker),
                "end": position(line, marker, after=True),
            },
        }

    assert query("hover", 3, "value", last=True)["contents"]["value"] == "value: T"
    assert query("definition", 3, "value", last=True) == expected(3, "value")
    assert query("definition", 8, "healthy") == expected(6, "healthy")
    assert query("hover", 10, "value", last=True)["contents"]["value"] == "value: T"
    assert query("definition", 10, "value", last=True) == expected(10, "value")
    inside = query("completion", 4, "this.", after=True)
    names = {item["label"] for item in inside["items"]}
    assert {"stored", "secret", "editing", "members", "partial", "healthy"} <= names, inside
    outside = query("completion", 9, "box.", after=True)
    assert "secret" not in {item["label"] for item in outside["items"]}, outside
    help_ = query("signatureHelp", 5, "healthy(", after=True)
    assert len(help_["signatures"]) == 1, help_
    assert "healthy(value: T): T" in help_["signatures"][0]["label"], help_

    damaged = source.replace("editing(value: T)", "editing(value: )")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 5}, "contentChanges": [{"text": damaged}]},
    )
    assert query("definition", 8, "healthy") is None
    assert query("hover", 10, "value", last=True)["contents"]["value"] == "value: T"
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    assert query("definition", 8, "healthy") == expected(6, "healthy")
    assert not (root / "target").exists()
