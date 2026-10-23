"""Recovered interface defaults keep original slots, binders and providers."""


def check(peer, root):
    path = root / "src/recoveredInterfaces.scoop"
    source = (
        "package recovered_interfaces\n"
        "interface Root<T> { fun echo(rootValue: T): T }\n"
        "interface Contract<T> : Root<T> {\n"
        "override fun echo(value: T): T { value. }\n"
        "private fun secret(): Boolean = true\n"
        "fun members() { this. }\n"
        "fun healthy(): Boolean = secret()\n"
        "fun partial() { healthy( }\n"
        "}\n"
        "class UsesDefault : Contract<Boolean> {}\n"
        "fun consume(input: UsesDefault): Boolean = input.echo(true)\n"
        "fun outside(input: Contract<Boolean>) { input. }\n"
        "class Outer<T> { interface Nested<U> { fun within(item: U) { item. } } }\n"
        "interface Mixed<T> { struct Nested<U>(val item: U) { fun within(item: U) { item. } } }\n"
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
    assert query("definition", 10, "echo") == expected(3, "echo")
    assert query("implementation", 1, "echo") == [expected(3, "echo")]
    assert query("definition", 6, "secret") == expected(4, "secret")
    for line in [12, 13]:
        assert query("hover", line, "item", last=True)["contents"]["value"] == "item: U"
    inside = query("completion", 5, "this.", after=True)
    assert {"echo", "secret", "healthy", "members"} <= {
        item["label"] for item in inside["items"]
    }, inside
    outside = query("completion", 11, "input.", after=True)
    assert "secret" not in {item["label"] for item in outside["items"]}, outside
    help_ = query("signatureHelp", 7, "healthy(", after=True)
    assert len(help_["signatures"]) == 1, help_
    assert "healthy(): Boolean" in help_["signatures"][0]["label"], help_

    damaged = source.replace("echo(value: T)", "echo(value: )")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 5}, "contentChanges": [{"text": damaged}]},
    )
    assert query("definition", 10, "echo") is None
    assert query("implementation", 1, "echo") == []
    assert query("hover", 13, "item", last=True)["contents"]["value"] == "item: U"
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    assert query("implementation", 1, "echo") == [expected(3, "echo")]
    assert query("definition", 10, "echo") == expected(3, "echo")
    assert not (root / "target").exists()
