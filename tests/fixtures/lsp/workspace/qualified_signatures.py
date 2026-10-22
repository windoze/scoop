"""Qualified call previews use dependency declarations and alias binders."""


def check(peer, root):
    provider = root / "dependency/src/qualifiedSignatures.scoop"
    declaration = (
        "package qualified_provider\n"
        "public enum Choice<T> {\n"
        "Empty, Full(val item: T, val flag: Boolean = true)\n"
        "public companion object Factory {\n"
        "public fun make(value: T): Choice<T> = Choice.Full(value)\n"
        "}\n"
        "}\n"
        "public typealias Alias<U> = Choice<U>\n"
        "public class Nest { public struct Box<Item>(val item: Item) }\n"
    )
    provider.write_text(declaration)
    path = root / "src/qualifiedSignatures.scoop"
    source = (
        "import qualified_provider.Choice\n"
        "import qualified_provider.Alias\n"
        "import qualified_provider.Nest\n"
        "fun qualifiedFirst() { Choice.Full<Boolean>( }\n"
        "fun qualifiedSecond() { Choice<Boolean>.Full( }\n"
        "fun qualifiedAlias() { Alias.Full<Boolean>( }\n"
        "fun qualifiedNested() { Nest.Box<Boolean>( }\n"
        "fun qualifiedCompanion() { Choice<Boolean>.make( }\n"
        "fun qualifiedCore() { Option.Some<Boolean>( }\n"
        "fun qualifiedShadow(Choice: Boolean) { Choice.Full( }\n"
    )
    path.write_text(source)
    for file, text in [(provider, declaration), (path, source)]:
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

    def help_at(line):
        character = source.splitlines()[line].rindex("(") + 1
        return peer.request(
            "textDocument/signatureHelp",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    for line, parameter in [
        (3, "item: T"),
        (4, "item: Boolean"),
        (5, "item: U"),
        (6, "item: Item"),
        (7, "value: Boolean"),
    ]:
        result = help_at(line)
        assert result and len(result["signatures"]) == 1, (line, result)
        assert result["signatures"][0]["parameters"][0]["label"] == parameter, (line, result)
    core = help_at(8)
    assert core and len(core["signatures"]) == 1, core
    assert core["signatures"][0]["label"].startswith("Some("), core
    assert help_at(9) is None
    before = help_at(3)
    assert before["signatures"][0]["parameters"][1]["label"] == "flag: Boolean = …", before

    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 4},
            "contentChanges": [
                {"text": declaration.replace("flag: Boolean = true", 'flag: String = "ready"')}
            ],
        },
    )
    changed = help_at(3)
    assert changed["signatures"][0]["parameters"][1]["label"] == "flag: String = …", changed
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    assert help_at(3) == before
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert not (root / "target").exists()
