"""Recovered enum methods retain variants, defaults and actual type identities."""


def check(peer, root):
    path = root / "src/recoveredEnums.scoop"
    source = (
        "package recovered_enums\n"
        "enum Choice<T> {\n"
        "Empty, Full(val item: T, val enabled: Boolean = true)\n"
        "fun editing(item: T): T { item. }\n"
        "fun members() { this. }\n"
        "fun construct() { Choice.Full<Boolean>( }\n"
        "fun healthy(item: T): T = item\n"
        "}\n"
        "fun consume(input: Choice<Boolean>): Boolean = input.healthy(true)\n"
        "class Outer<T> { enum Nested<U> { Value(U)\n"
        "fun within(item: U) { item. } } }\n"
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

    assert query("hover", 3, "item", last=True)["contents"]["value"] == "item: T"
    assert query("definition", 3, "item", last=True) == expected(3, "item")
    assert query("definition", 8, "healthy") == expected(6, "healthy")
    definition = query("typeDefinition", 8, "input")
    assert definition == expected(1, "Choice"), definition
    assert query("hover", 10, "item", last=True)["contents"]["value"] == "item: U"
    inside = query("completion", 4, "this.", after=True)
    assert {"editing", "members", "construct", "healthy"} <= {
        item["label"] for item in inside["items"]
    }, inside
    help_ = query("signatureHelp", 5, "Choice.Full<Boolean>(", after=True)
    assert help_ and len(help_["signatures"]) == 1, help_
    assert help_["signatures"][0]["label"] == "Full(item: T, enabled: Boolean = …): Choice<T>", (
        help_
    )

    damaged = source.replace("val item: T", "val item: ")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 5}, "contentChanges": [{"text": damaged}]},
    )
    assert query("definition", 8, "healthy") is None
    assert query("hover", 10, "item", last=True)["contents"]["value"] == "item: U"
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    assert query("definition", 8, "healthy") == expected(6, "healthy")
    assert not (root / "target").exists()
