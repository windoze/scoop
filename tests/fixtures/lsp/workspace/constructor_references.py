"""Constructor calls retain source identities and their own parameter documents."""


def check(peer, root):
    provider = root / "dependency/src/constructors.scoop"
    library = (
        "package construction\n"
        "/// @param value The imported constructor input.\n"
        "public class Remote<T> public constructor(\n"
        "/// The imported stored property.\n"
        "public val value: T)\n"
        "public enum Signal { Idle,\n"
        "/// @param value The variant input.\n"
        "Data(/** The variant stored payload. */ val value: Boolean) }\n"
    )
    path = root / "src/constructors.scoop"
    source = (
        "import construction.Remote as Selected\n"
        "import construction.Signal\n"
        "import construction.Signal.Data as Make\n"
        "import construction.Signal.Idle as Waiting\n"
        "/// @param value The local constructor input.\n"
        "class ConstructorLocalBox(\n"
        "/// The local stored property.\n"
        "val value: Boolean)\n"
        "struct ConstructorValue(val value: Boolean)\n"
        "class ConstructorOverloaded {\n"
        "    constructor(value: Boolean) {}\n"
        "    constructor(value: String) {}\n"
        "}\n"
        "fun constructorReferences(): Boolean {\n"
        "    val local = ConstructorLocalBox(value = true)\n"
        "    val structure = ConstructorValue(value = true)\n"
        "    val boolean = ConstructorOverloaded(value = true)\n"
        '    val string = ConstructorOverloaded(value = "text")\n'
        "    val localFactory: (Boolean) -> ConstructorOverloaded = ::ConstructorOverloaded\n"
        "    val remote = Selected(value = true)\n"
        "    val variant = Make(value = true)\n"
        "    val waiting = Waiting\n"
        "    val factory: (Boolean) -> Selected<Boolean> = ::Selected\n"
        "    return local.value\n"
        "}\n"
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

    def query(needle, name, method, **extra):
        prefix = source[: source.index(needle) + needle.index(name)]
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {
                    "line": prefix.count("\n"),
                    "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
                },
                **extra,
            },
        )

    def parameter(needle, expected, file, line):
        result = query(needle, "value", "hover")
        signature, documentation = result["contents"]
        assert signature["value"] == "value: Boolean", result
        assert documentation == expected, result
        target = query(needle, "value", "definition")
        assert target["uri"] == file.as_uri(), target
        assert target["range"]["start"]["line"] == line, target
        references = query(needle, "value", "references", context={"includeDeclaration": True})
        assert len(references) == 2, references
        assert target in references, references

    parameter("ConstructorLocalBox(value = true)", "The local constructor input.", path, 7)
    structure = query("ConstructorValue(value = true)", "value", "definition")
    assert structure["range"]["start"]["line"] == 8, structure
    assert structure["range"]["end"]["character"] - structure["range"]["start"]["character"] == 5
    references = query(
        "ConstructorValue(value = true)",
        "value",
        "references",
        context={"includeDeclaration": True},
    )
    assert len(references) == 2 and structure in references, references
    declared = query(
        "ConstructorValue(val value", "value", "references", context={"includeDeclaration": True}
    )
    assert references == declared, declared
    for needle, line in [
        ("ConstructorOverloaded(value = true)", 10),
        ('ConstructorOverloaded(value = "text")', 11),
    ]:
        target = query(needle, "ConstructorOverloaded", "definition")
        assert target["uri"] == path.as_uri(), target
        assert target["range"] == {
            "start": {"line": line, "character": 4},
            "end": {"line": line, "character": 15},
        }, target
    selected = query("ConstructorOverloaded(value = true)", "ConstructorOverloaded", "definition")
    assert query("::ConstructorOverloaded", "ConstructorOverloaded", "definition") == selected
    references = query(
        "constructor(value: Boolean)",
        "constructor",
        "references",
        context={"includeDeclaration": True},
    )
    assert len(references) == 3 and selected in references, references
    parameter("Selected(value = true)", "The imported constructor input.", provider, 4)
    parameter("Make(value = true)", "The variant input.", provider, 7)
    for needle, name in [
        ("Selected(value", "Selected"),
        ("::Selected", "Selected"),
        ("Make(value", "Make"),
        ("waiting = Waiting", "Waiting"),
        ("Data as Make", "Data"),
    ]:
        target = query(needle, name, "definition")
        assert target["uri"] == provider.as_uri(), target

    updated = library.replace("The imported constructor input.", "Updated constructor input.")
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 4},
            "contentChanges": [{"text": updated}],
        },
    )
    parameter("Selected(value = true)", "Updated constructor input.", provider, 4)
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    parameter("Selected(value = true)", "The imported constructor input.", provider, 4)
