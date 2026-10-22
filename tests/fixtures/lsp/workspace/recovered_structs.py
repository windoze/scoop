"""Recovered struct methods keep real fields, constructors and nested owners."""


def check(peer, root):
    path = root / "src/recoveredStructs.scoop"
    source = (
        "package recovered_structs\n"
        "struct Value<T>(val item: T, val enabled: Boolean = true) {\n"
        "fun editing(item: T): T { item. }\n"
        "fun members() { this. }\n"
        "fun construct() { Value<Boolean>( }\n"
        "fun healthy(item: T): T = item\n"
        "}\n"
        "fun consume(input: Value<Boolean>): Boolean = input.healthy(input.item)\n"
        "class Box { struct Nested<T>(val item: T) { fun within(item: T) { item. } } }\n"
        "struct Other(val flag: Boolean) { class Nested<T> { fun within(item: T) { item. } } }\n"
        "struct Plain<T>(val item: T, val enabled: Boolean = true)\n"
        "fun plainConstructor() { Plain<Boolean>( }\n"
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

    assert query("hover", 2, "item", last=True)["contents"]["value"] == "item: T"
    assert query("definition", 2, "item", last=True) == expected(2, "item")
    assert query("definition", 7, "healthy") == expected(5, "healthy")
    assert query("definition", 7, "item", last=True) == expected(1, "item")
    for line in [8, 9]:
        assert query("hover", line, "item", last=True)["contents"]["value"] == "item: T"
    result = query("completion", 3, "this.", after=True)
    names = {item["label"] for item in result["items"]}
    assert {"item", "enabled", "editing", "members", "healthy"} <= names, result
    help_ = query("signatureHelp", 4, "Value<Boolean>(", after=True)
    assert len(help_["signatures"]) == 1, help_
    assert help_["signatures"][0]["label"] == ("Value(item: T, enabled: Boolean = …): Value<T>"), (
        help_
    )
    plain = query("signatureHelp", 11, "Plain<Boolean>(", after=True)
    assert plain["signatures"][0]["parameters"] == help_["signatures"][0]["parameters"], plain

    damaged = source.replace("val item: T,", "val item: ,", 1)
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 5}, "contentChanges": [{"text": damaged}]},
    )
    assert query("definition", 7, "healthy") is None
    assert query("hover", 9, "item", last=True)["contents"]["value"] == "item: T"
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    assert query("definition", 7, "healthy") == expected(5, "healthy")
    assert not (root / "target").exists()
