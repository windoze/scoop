"""Recovered singleton methods retain the real owner and generic host."""


def check(peer, root):
    path = root / "src/recoveredObjects.scoop"
    source = (
        "package recovered_objects\n"
        "class Host<T> {\n"
        "companion object Factory {\n"
        "val enabled: Boolean = true\n"
        "fun editing(item: T): T { item. }\n"
        "fun members() { this. }\n"
        "fun preview(item: T) { healthy(item, }\n"
        "fun healthy(item: T, flag: Boolean = true): T = item\n"
        "}\n"
        "}\n"
        "object Standalone {\n"
        "val enabled: Boolean = true\n"
        "fun editing() { this. }\n"
        "fun healthy(): Boolean = enabled\n"
        "object Nested { fun <U> inner(value: U) { value. } }\n"
        "}\n"
        "fun consume(): Boolean = Host<Boolean>.healthy(true)\n"
        "fun useSingleton(): Boolean = Standalone.healthy()\n"
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
            {"textDocument": {"uri": uri}, "position": position(line, marker, last, after)},
        )

    def expected(line, marker):
        return {
            "uri": uri,
            "range": {
                "start": position(line, marker),
                "end": position(line, marker, after=True),
            },
        }

    assert query("hover", 4, "item", last=True)["contents"]["value"] == "item: T"
    assert query("definition", 4, "item", last=True) == expected(4, "item")
    assert query("definition", 16, "healthy") == expected(7, "healthy")
    assert query("definition", 17, "healthy") == expected(13, "healthy")
    assert query("definition", 13, "enabled") == expected(11, "enabled")
    assert query("hover", 14, "value", last=True)["contents"]["value"] == "value: U"
    definition = query("typeDefinition", 12, "this")
    assert definition == expected(10, "Standalone"), definition
    assert query("hover", 12, "this")["contents"]["value"] == "Standalone"
    inside = query("completion", 5, "this.", after=True)
    assert {"enabled", "editing", "members", "preview", "healthy"} <= {
        item["label"] for item in inside["items"]
    }, inside
    help_ = query("signatureHelp", 6, "healthy(item,", after=True)
    assert help_ and len(help_["signatures"]) == 1, help_
    assert help_["signatures"][0]["parameters"][1]["label"] == "flag: Boolean = …", help_

    damaged = source.replace("fun healthy(item: T,", "fun healthy(item: ,")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 5}, "contentChanges": [{"text": damaged}]},
    )
    assert query("definition", 16, "healthy") is None
    assert query("definition", 17, "healthy") == expected(13, "healthy")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    assert query("definition", 16, "healthy") == expected(7, "healthy")
    assert not (root / "target").exists()
