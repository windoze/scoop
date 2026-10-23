"""Type hovers use original declarations, application binders, and M60 targets."""


def check(peer, root):
    provider = root / "dependency/src/typeHovers.scoop"
    library = (
        "package typehovers\n"
        "/// The imported carrier.\n"
        "public struct Carrier<Element>(val value: Element)\n"
        "/// **Imported alias** uses [Carrier].\n"
        "/// @param Entry The imported entry type.\n"
        "public typealias Remote<Entry> = Carrier<Entry>\n"
    )
    path = root / "src/typeHovers.scoop"
    source = (
        "import typehovers.Remote as Imported\n"
        "import typehovers.Carrier\n"
        "/// Local **box documentation**.\n"
        "/// @param Item The local item type.\n"
        "struct LocalBox<Item>(val value: Item)\n"
        "/// The local alias uses [LocalBox].\n"
        "/// @param Value The wrapped type.\n"
        "typealias LocalAlias<Value> = LocalBox<Value>\n"
        "/// A generic function.\n"
        "/// @param 使用 The query type.\n"
        "fun <使用> typedHover(value: LocalAlias<使用>): LocalBox<使用> = value\n"
        "fun remoteHover(value: Imported<Boolean>): Carrier<Boolean> = value\n"
        "fun <Outer> nestedTypes(input: LocalBox<Outer>): LocalBox<Outer> { fun <Inner> nested(value: LocalBox<Inner>): LocalBox<Inner> { typealias Local = LocalBox<Inner>; val copy: Local = value; return copy }; return nested(input) }\n"
        "fun damagedTypes(value: LocalBox<Boolean>): LocalBox<Boolean> { value. }\n"
    )
    for file, text in [(provider, library), (path, source)]:
        file.write_text(text)
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

    def query(line, name, *, last=False, method="hover"):
        text = source.splitlines()[line]
        start = text.rindex(name) if last else text.index(name)
        result = peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": start},
            },
        )
        if method == "hover":
            assert result["range"]["start"] == {"line": line, "character": start}, result
        return result

    def hover(line, name, *, last=False):
        result = query(line, name, last=last)
        content = result["contents"]
        return (
            (content[0]["value"], content[1])
            if isinstance(content, list)
            else (content["value"], None)
        )

    assert hover(4, "LocalBox") == (
        "struct LocalBox<Item>",
        "Local **box documentation**.\n\n**@param** Item — The local item type.",
    )
    for line, name, expected in [
        (4, "Item", "The local item type."),
        (7, "Value", "The wrapped type."),
        (10, "使用", "The query type."),
    ]:
        assert hover(line, name) == (f"type parameter {name}", expected)
    assert hover(7, "LocalAlias")[0] == "typealias LocalAlias<Value> = LocalBox<Value>"
    signature, docs = hover(10, "LocalAlias")
    assert signature == "typealias LocalAlias<Value> = LocalBox<Value>\ntype: LocalBox<使用>"
    assert f"[LocalBox](<{path.as_uri()}#L5>)" in docs, docs
    assert hover(10, "使用", last=True) == ("type parameter 使用\ntype: 使用", "The query type.")
    assert (
        hover(12, "Local", last=True)[0]
        == "typealias Local = LocalBox<Inner>\ntype: LocalBox<Inner>"
    )
    assert hover(13, "LocalBox")[0] == "struct LocalBox<Item>\ntype: LocalBox<Boolean>"

    def imported(parameter="Entry", title="Imported alias", value="Boolean"):
        for name in ["Remote", "Imported"]:
            signature, docs = hover(0, name)
            assert signature == f"typealias Remote<{parameter}> = Carrier<{parameter}>", signature
            assert f"**{title}**" in docs, docs
        signature, docs = hover(11, "Imported")
        assert (
            signature
            == f"typealias Remote<{parameter}> = Carrier<{parameter}>\ntype: Carrier<{value}>"
        ), signature
        assert f"[Carrier](<{provider.as_uri()}#L3>)" in docs, docs
        target = query(11, "Imported", method="definition")
        assert target["uri"] == provider.as_uri(), target
        assert target["range"]["start"]["line"] == 5, target

    imported()
    assert hover(1, "Carrier") == ("struct Carrier<Element>", "The imported carrier.")
    original = source
    source = source.replace(
        "Imported<Boolean>): Carrier<Boolean>", "Imported<String>): Carrier<String>"
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 4},
            "contentChanges": [{"text": source}],
        },
    )
    imported(value="String")
    updated = library.replace("Imported alias", "Updated alias").replace("Entry", "元素")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 7},
            "contentChanges": [{"text": updated}],
        },
    )
    imported("元素", "Updated alias", "String")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    imported(value="String")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    source = original
    imported()
    assert not (root / "target").exists()
    assert not (root / "dependency/target").exists()
