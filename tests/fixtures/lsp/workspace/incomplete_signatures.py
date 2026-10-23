"""Unfinished calls retain real candidates, syntax slots and declaration types."""


def check(peer, root):
    path = root / "src/incomplete_signatures.scoop"
    prefix = (
        "fun partialPick(flag: Boolean, text: String): String = text\n"
        "fun partialPick(flag: String, text: String): String = text\n"
        "fun <Item> partialGather(value: Item, vararg rest: Item): Item = value\n"
        "struct PartialBox<Element>(val value: Element)\n"
    )
    uri = path.as_uri()
    path.write_text(prefix)
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": prefix}},
    )

    def help_for(body, version, marker=None):
        text = prefix + body
        peer.notify(
            "textDocument/didChange",
            {"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]},
        )
        column = len(body) if marker is None else body.rindex(marker)
        character = len(body[:column].encode("utf-16-le")) // 2
        return peer.request(
            "textDocument/signatureHelp",
            {"textDocument": {"uri": uri}, "position": {"line": 4, "character": character}},
        )

    result = help_for("fun partialEditing() { partialPick(", 2)
    assert {item["label"] for item in result["signatures"]} == {
        "partialPick(flag: Boolean, text: String): String",
        "partialPick(flag: String, text: String): String",
    }, result
    assert result.get("activeSignature") is None, result
    assert all(item["activeParameter"] == 0 for item in result["signatures"]), result

    result = help_for("fun partialEditing() { partialPick(true,", 3)
    assert len(result["signatures"]) == 1, result
    assert result["signatures"][0]["label"] == "partialPick(flag: Boolean, text: String): String"
    assert result["signatures"][0]["activeParameter"] == 1, result

    result = help_for('fun partialEditing() { partialPick(text = "中文😀", flag = ) }', 4, ")")
    assert len(result["signatures"]) == 2, result
    assert all(item["activeParameter"] == 0 for item in result["signatures"]), result

    result = help_for("fun partialEditing() { partialGather(true,", 5)
    assert result["signatures"][0]["label"] == "partialGather(value: Item, vararg rest: Item): Item"
    assert result["signatures"][0]["activeParameter"] == 1, result

    result = help_for("fun partialEditing(action: (Boolean) -> String) { action(", 6)
    assert result["signatures"][0]["label"] == "action(Boolean): String", result

    result = help_for("fun partialEditing() { true.equals(", 7)
    assert result["signatures"][0]["label"] == "equals(other: Boolean): Boolean", result

    result = help_for("fun partialEditing() { PartialBox<Boolean>(", 8)
    assert result["signatures"][0]["label"] == "PartialBox(value: Element): PartialBox<Element>", (
        result
    )

    result = help_for("fun partialEditing() { partialGather(", 9)
    assert result["signatures"][0]["activeParameter"] == 0, result

    assert (
        help_for('fun partialEditing() { partialPick(true, "fixed") }', 10, "partialPick") is None
    )
    for version, body in enumerate(
        [
            "fun partialEditing() { val value = partialPick(true,",
            "fun partialEditing(): String { return partialPick(true,",
            "fun partialEditing(): String { return@partialEditing partialPick(true,",
            "fun partialEditing() { { val value = true; partialPick(value,",
        ],
        11,
    ):
        result = help_for(body, version)
        assert (
            result["signatures"][0]["label"] == "partialPick(flag: Boolean, text: String): String"
        ), result
        assert result["signatures"][0]["activeParameter"] == 1, result
    assert help_for("fun partialEditing() { return@missing partialPick(", 15) is None
    assert help_for("fun partialEditing() { val value: Missing = partialPick(", 16) is None
    result = help_for(
        "fun partialEditing(value: Any) { if (value is String) { partialPick(value,", 17
    )
    assert len(result["signatures"]) == 1, result
    assert result["signatures"][0]["label"] == "partialPick(flag: String, text: String): String"
    result = help_for("fun partialEditing() { if (partialPick(", 18)
    assert len(result["signatures"]) == 2, result
    assert help_for('fun partialEditing() { if ("invalid") { partialPick(', 19) is None
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
