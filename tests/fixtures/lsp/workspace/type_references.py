"""Type names retain declaration identities across aliases, binders and updates."""


def check(peer, root):
    provider = root / "dependency/src/navigationTypes.scoop"
    library = (
        "package typednav\n"
        "/// A generic source type.\n"
        "public struct 原值<T>(val value: T)\n"
        "public typealias Wrapped<T> = 原值<T>\n"
        "public class Outer { public class Inner {} }\n"
    )
    path = root / "src/navigationTypes.scoop"
    source = (
        "import typednav.原值 as 本地\n"
        "import typednav.Wrapped\n"
        "import typednav.Outer\n"
        "fun useType(value: 本地<Boolean>): Wrapped<Boolean> { val copy: 本地<Boolean> = value; return copy }\n"
        "fun nestedType(value: Outer.Inner): Outer.Inner = value\n"
        "fun <本地> shadowType(value: 本地): 本地 = value\n"
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

    def request(method, file, text, line, name, last=False):
        content = text.splitlines()[line]
        params = {
            "textDocument": {"uri": file.as_uri()},
            "position": {
                "line": line,
                "character": content.rindex(name) if last else content.index(name),
            },
        }
        if method == "references":
            params["context"] = {"includeDeclaration": True}
        return peer.request(f"textDocument/{method}", params)

    def definition(line, name, expected_line, expected_name, last=False):
        result = request("definition", path, source, line, name, last)
        start = library.splitlines()[expected_line].index(expected_name)
        assert result == {
            "uri": provider.as_uri(),
            "range": {
                "start": {"line": expected_line, "character": start},
                "end": {"line": expected_line, "character": start + len(expected_name)},
            },
        }, result

    definition(0, "原值", 2, "原值")
    definition(0, "本地", 2, "原值")
    definition(3, "本地", 2, "原值")
    definition(3, "Wrapped", 3, "Wrapped")
    definition(4, "Outer", 4, "Outer")
    definition(4, "Inner", 4, "Inner", last=True)
    generic = request("definition", path, source, 5, "本地", last=True)
    assert generic["uri"] == path.as_uri(), generic
    assert generic["range"]["start"] == {"line": 5, "character": 5}, generic
    assert request("definition", path, source, 3, "Boolean") is None

    def nominal_references():
        return request("references", provider, library, 2, "原值")

    references = nominal_references()
    assert len(references) == 5, references
    assert sum(item["uri"] == provider.as_uri() for item in references) == 2, references
    aliases = request("references", path, source, 0, "本地")
    assert len(aliases) == 3 and all(item["uri"] == path.as_uri() for item in aliases), aliases
    parameters = request("references", path, source, 5, "本地")
    assert len(parameters) == 3, parameters
    assert all(item["range"]["start"]["line"] == 5 for item in parameters), parameters
    wrapped = request("references", path, source, 3, "Wrapped")
    assert len(wrapped) == 3, wrapped
    assert any(item["uri"] == provider.as_uri() for item in wrapped), wrapped

    updated = source.replace("val copy: 本地<Boolean>", "val copy")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 7},
            "contentChanges": [{"text": updated}],
        },
    )
    assert len(nominal_references()) == 4
    assert len(request("references", path, updated, 0, "本地")) == 2
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    assert nominal_references() == references
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 4},
            "contentChanges": [{"text": "// updated\n" + library}],
        },
    )
    shifted = request("definition", path, source, 3, "本地")
    assert shifted["range"]["start"]["line"] == 3, shifted
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    definition(3, "本地", 2, "原值")
    assert not (provider.parent.parent / "target").exists()
