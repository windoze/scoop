"""Incomplete callable arguments use the original call mapping and inference."""


def check(peer, root):
    path = root / "src/callableArgumentRecovery.scoop"
    source = (
        "fun <T, R> argumentMap(seed: T, block: (T) -> R): R = block(seed)\n"
        "fun <T, R> argumentWith(seed: T, block: T.() -> R): R = seed.block()\n"
        "fun argumentChoice(value: String, next: Boolean): Boolean = next\n"
        "fun argumentChoice(value: Boolean, next: Boolean): Boolean = next\n"
        "fun argumentAmbiguous(block: (String) -> Boolean) {}\n"
        "fun argumentAmbiguous(block: (Boolean) -> Boolean) {}\n"
        'fun argumentHover() { argumentMap(seed = "known", block = { 参数 -> return@argumentMap 参数. }) }\n'
        'fun argumentSignature() { argumentMap("known", { 参数 -> argumentChoice(参数, }) }\n'
        'fun argumentAnonymous() { argumentMap("explicit", fun(value: String) { return value. }) }\n'
        'fun argumentReceiver() { argumentWith("receiver", { this. }) }\n'
        "fun argumentCapture(value: String) { argumentWith(true, { value. }) }\n"
        "fun argumentUncertain(value: Boolean) { argumentAmbiguous({ value -> value. }) }\n"
        "fun argumentUnknown(value: Boolean) { argumentMap(, { value -> value. }) }\n"
        "fun trailingUse(block: (String) -> Boolean) {}\n"
        'fun trailingHover() { argumentMap("known") { 参数 -> return@argumentMap 参数. } }\n'
        'fun trailingSignature() { argumentMap("known") { 参数 -> argumentChoice(参数, } }\n'
        "fun trailingOnly() { trailingUse { 参数 -> 参数. } }\n"
        'fun trailingReceiver() { argumentWith("receiver") { this. } }\n'
        "fun trailingUncertain(value: Boolean) { argumentAmbiguous { value -> value. } }\n"
        "fun trailingUnknown(value: Boolean) { argumentMap(,) { value -> value. } }\n"
        "fun argumentFactory(): ((String) -> Boolean) -> Boolean = { true }\n"
        "fun trailingGrouped() { (argumentFactory()) { 参数 -> 参数. } }\n"
    )
    path.write_text(source)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": source}},
    )

    def query(method, line, marker, after=False):
        offset = source.splitlines()[line].rindex(marker)
        if after:
            offset += len(marker)
        return peer.request(
            method,
            {"textDocument": {"uri": uri}, "position": {"line": line, "character": offset}},
        )

    def check_type(expected):
        for line in [6, 14]:
            hover = query("textDocument/hover", line, "参数.")
            assert hover["contents"]["value"] == f"参数: {expected}", hover
        for line in [7, 15]:
            signature = query("textDocument/signatureHelp", line, ",", after=True)
            assert signature and len(signature["signatures"]) == 1, signature
            assert signature["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", (
                signature
            )

    check_type("String")
    for line, marker, definition_marker in [
        (6, "参数.", "参数 ->"),
        (8, "value.", "value:"),
        (10, "value.", "value:"),
        (14, "参数.", "参数 ->"),
        (16, "参数.", "参数 ->"),
        (21, "参数.", "参数 ->"),
    ]:
        definition = query("textDocument/definition", line, marker)
        assert definition["uri"] == uri, definition
        assert definition["range"]["start"] == {
            "line": line,
            "character": source.splitlines()[line].index(definition_marker),
        }, definition
    completion = query("textDocument/completion", 6, "参数.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    for line, marker, expected in [
        (8, "value.", "value: String"),
        (9, "this.", "String"),
        (10, "value.", "value: String"),
        (16, "参数.", "参数: String"),
        (17, "this.", "String"),
        (21, "参数.", "参数: String"),
    ]:
        hover = query("textDocument/hover", line, marker)
        assert hover["contents"]["value"] == expected, hover
    for line, active in [(14, 1), (16, 0), (21, 0)]:
        signature = query("textDocument/signatureHelp", line, "参数 ->")
        assert signature and len(signature["signatures"]) == 1, signature
        assert signature["signatures"][0]["activeParameter"] == active, signature
    for line in [11, 12, 18, 19]:
        assert query("textDocument/hover", line, "value.") is None
    disk_source = source
    source = source.replace('"known"', "true")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
