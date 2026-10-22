"""Property queries use logical bindings across reads, writes and source libraries."""

SOURCE_DECLARATIONS = (
    "public class PropertyBox<T> public constructor(public var item: T)\n"
    "public var <T> PropertyBox<T>.property: T get() = item\n"
    "set(value) { item = value }\n"
    "public const val PROPERTY_FLAG: Boolean = true\n"
)


def check(peer, root):
    path = root / "src/propertyReferences.scoop"
    text = (
        "const val PROPERTY_FLAG: Boolean = true\n"
        "class PropertyBox<T>(var 属性: T)\n"
        "var <T> PropertyBox<T>.property: T get() = 属性\n"
        "set(value) { 属性 = value }\n"
        "fun propertyUse(value: PropertyBox<Boolean>): Boolean {\n"
        "    value.属性 = PROPERTY_FLAG\n"
        "    value.property = value.属性\n"
        "    return value.property\n"
        "}\n"
    )
    path.write_text(text)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": text}},
    )
    for line, name, definition, expected in [
        (0, "PROPERTY_FLAG", 0, "val PROPERTY_FLAG: Boolean"),
        (1, "属性", 1, "var 属性: T"),
        (2, "property", 2, "var <T> PropertyBox<T>.property: T"),
        (5, "属性", 1, "var 属性: Boolean"),
        (5, "PROPERTY_FLAG", 0, "val PROPERTY_FLAG: Boolean"),
        (6, "property", 2, "var property: Boolean"),
        (6, "属性", 1, "var 属性: Boolean"),
        (7, "property", 2, "var property: Boolean"),
    ]:
        position = {"line": line, "character": text.splitlines()[line].index(name)}
        params = {"textDocument": {"uri": uri}, "position": position}
        hover = peer.request("textDocument/hover", params)
        assert hover["contents"]["value"] == expected, hover
        target = peer.request("textDocument/definition", params)
        assert target["uri"] == uri, target
        assert target["range"]["start"]["line"] == definition, target
        declaration_line = text.splitlines()[definition]
        assert target["range"]["start"]["character"] == declaration_line.index(name), target
        assert target["range"]["end"]["character"] == declaration_line.index(name) + len(name), (
            target
        )
    assert not peer.diagnostics[uri]["diagnostics"], peer.diagnostics[uri]


def check_imported(peer, root, library):
    path = root / "src/importedProperties.scoop"
    text = (
        "import shared.PropertyBox\nimport shared.property\n"
        "import shared.PROPERTY_FLAG as enabled\n"
        "fun importedPropertyUse(value: PropertyBox<Boolean>): Boolean {\n"
        "    value.item = enabled\n"
        "    value.property = value.item\n"
        "    return value.property\n"
        "}\n"
    )
    path.write_text(text)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": text}},
    )
    for line, name, expected in [
        (4, "item", "var item: Boolean"),
        (4, "enabled", "val PROPERTY_FLAG: Boolean"),
        (5, "property", "var property: Boolean"),
        (5, "item", "var item: Boolean"),
        (6, "property", "var property: Boolean"),
    ]:
        params = {
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": text.splitlines()[line].index(name)},
        }
        hover = peer.request("textDocument/hover", params)
        assert hover["contents"]["value"] == expected, hover
        target = peer.request("textDocument/definition", params)
        assert target["uri"] == library.as_uri(), target
        references = peer.request(
            "textDocument/references", {**params, "context": {"includeDeclaration": True}}
        )
        counts = {"enabled": 2, "property": 4, "item": 3}
        assert len(references) == counts[name], references
        assert sum(location["uri"] == library.as_uri() for location in references) == (
            0 if name == "enabled" else 1
        ), references
        assert all(location["uri"] in (uri, library.as_uri()) for location in references), (
            references
        )
    for line, name, original in [
        (1, "property", "property"),
        (2, "PROPERTY_FLAG", "PROPERTY_FLAG"),
        (2, "enabled", "PROPERTY_FLAG"),
    ]:
        params = {
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": text.splitlines()[line].index(name)},
        }
        hover = peer.request("textDocument/hover", params)
        expected = (
            "var <T> PropertyBox<T>.property: T"
            if name == "property"
            else "val PROPERTY_FLAG: Boolean"
        )
        assert hover["contents"]["value"] == expected, hover
        target = peer.request(
            "textDocument/definition",
            params,
        )
        assert target["uri"] == library.as_uri(), target
        selected = target["range"]
        source_line = library.read_text().splitlines()[selected["start"]["line"]]
        assert (
            source_line[selected["start"]["character"] : selected["end"]["character"]] == original
        ), target
    assert not peer.diagnostics[uri]["diagnostics"], peer.diagnostics[uri]
