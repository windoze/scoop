"""Implementation navigation follows checked, shared nominal relationships."""


def check(peer, root):
    provider = root / "dependency/src/typeImplementations.scoop"
    library = (
        "package hierarchy\n"
        "public interface 契约<T> {}\n"
        "public interface Middle<T> : 契约<T> {}\n"
        "public abstract class Base<T> public constructor() : Middle<T> {}\n"
        "public class Existing public constructor() : Base<Boolean>() {}\n"
    )
    path = root / "src/typeImplementations.scoop"
    source = (
        "package hierarchy_use\n"
        "import hierarchy.契约 as Contract\n"
        "import hierarchy.Middle\n"
        "import hierarchy.Base\n"
        "class Local : Base<Boolean>() {}\n"
        "struct Value(val flag: Boolean) : Contract<Boolean> {}\n"
        "object Singleton : Middle<Boolean> {}\n"
        "typealias Saved = Contract<Boolean>\n"
        "fun usingContract(value: Saved): Contract<Boolean> = value\n"
        "fun damagedContract(value: Contract<Boolean>) { value. }\n"
    )
    other = root / "src/unrelatedImplementations.scoop"
    for file, text in [
        (provider, library),
        (path, source),
        (other, "package unrelated\ninterface 契约<T> {}\nclass Wrong : 契约<Boolean> {}\n"),
    ]:
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

    def query(file, text, line, name):
        return peer.request(
            "textDocument/implementation",
            {
                "textDocument": {"uri": file.as_uri()},
                "position": {"line": line, "character": text.splitlines()[line].index(name)},
            },
        )

    def location(file, text, line, name):
        column = text.splitlines()[line].index(name)
        return {
            "uri": file.as_uri(),
            "range": {
                "start": {"line": line, "character": column},
                "end": {"line": line, "character": column + len(name)},
            },
        }

    expected = [
        location(provider, library, 2, "Middle"),
        location(provider, library, 3, "Base"),
        location(provider, library, 4, "Existing"),
        location(path, source, 4, "Local"),
        location(path, source, 5, "Value"),
        location(path, source, 6, "Singleton"),
    ]

    def verify(result, wanted):
        assert result is not None, wanted
        assert len(result) == len(wanted), (result, wanted)
        assert all(item in result for item in wanted), (result, wanted)

    verify(query(provider, library, 1, "契约"), expected)
    verify(query(path, source, 1, "契约"), expected)
    verify(query(path, source, 1, "Contract"), expected)
    verify(query(path, source, 5, "Contract"), expected)
    verify(query(path, source, 8, "Saved"), expected)
    verify(query(path, source, 3, "Base"), [expected[2], expected[3]])
    assert query(path, source, 5, "Value") == []
    assert query(path, source, 8, "usingContract") is None

    updated = source.replace("class Local : Base<Boolean>() {}", "class Local {}")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 2},
            "contentChanges": [{"text": updated}],
        },
    )
    verify(query(provider, library, 1, "契约"), [item for item in expected if item != expected[3]])
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    verify(query(provider, library, 1, "契约"), expected)

    updated_library = library.replace(" : Base<Boolean>()", "")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 2},
            "contentChanges": [{"text": updated_library}],
        },
    )
    verify(query(path, source, 1, "Contract"), [item for item in expected if item != expected[2]])
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    verify(query(path, source, 1, "Contract"), expected)
    assert not (provider.parent.parent / "target").exists()
