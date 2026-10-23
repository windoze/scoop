"""Constructor navigation and source type names share successful binding facts."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/constructor_names.scoop"
    library = (
        "package constructornameapi\n"
        "public class RemoteName<T> public constructor(value: T)\n"
        "public class OtherRemoteName<T> public constructor(value: T)\n"
        "public typealias RemoteAlias<T> = RemoteName<T>\n"
    )
    path = root / "src/constructor_names.scoop"
    source = (
        "package constructornameconsumer\n"
        "import constructornameapi.RemoteName as ImportedName\n"
        "import constructornameapi.RemoteAlias as ImportedAlias\n"
        "struct LocalName<T>(val value: T)\n"
        "struct OtherLocalName<T>(val value: T)\n"
        "typealias LocalAlias<T> = LocalName<T>\n"
        "fun constructorNames() {\n"
        "    val direct = LocalName(true)\n"
        "    val alias = /* 🌟 */ LocalAlias(true)\n"
        "    val remote = ImportedName(true)\n"
        "    val imported = ImportedAlias(true)\n"
        "    val localFactory: (Boolean) -> LocalAlias<Boolean> = ::LocalAlias\n"
        "    val remoteFactory: (Boolean) -> ImportedAlias<Boolean> = ::ImportedAlias\n"
        "}\n"
    )
    texts = {path: source, provider: library}
    for file, text in texts.items():
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

    def position(file, needle, name):
        text = texts[file]
        prefix = text[: text.index(needle) + needle.index(name)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(file, needle, name, method, **extra):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": file.as_uri()},
                "position": position(file, needle, name),
                **extra,
            },
        )

    def references(file, needle, name, count):
        result = query(file, needle, name, "references", context={"includeDeclaration": True})
        assert result is not None and len(result) == count, (needle, result)
        return result

    def definition(needle, name, file, declaration, target):
        result = query(path, needle, name, "definition")
        start = position(file, declaration, target)
        assert result == {
            "uri": file.as_uri(),
            "range": {
                "start": start,
                "end": {**start, "character": start["character"] + len(target)},
            },
        }, (needle, result)

    assert published(peer, path.as_uri(), 1) == []
    assert published(peer, provider.as_uri(), 1) == []
    local = references(path, "typealias LocalAlias", "LocalAlias", 4)
    assert references(path, "LocalAlias(true)", "LocalAlias", 4) == local
    assert references(path, "::LocalAlias", "LocalAlias", 4) == local
    references(path, "struct LocalName", "LocalName", 3)
    imported = references(path, "as ImportedAlias", "ImportedAlias", 4)
    assert references(path, "ImportedAlias(true)", "ImportedAlias", 4) == imported
    assert references(path, "::ImportedAlias", "ImportedAlias", 4) == imported
    references(provider, "typealias RemoteAlias", "RemoteAlias", 5)
    references(path, "as ImportedName", "ImportedName", 2)
    definition("LocalAlias(true)", "LocalAlias", path, "struct LocalName", "LocalName")
    definition("::LocalAlias", "LocalAlias", path, "struct LocalName", "LocalName")
    definition("ImportedAlias(true)", "ImportedAlias", provider, "class RemoteName", "RemoteName")

    texts[provider] = library.replace("= RemoteName<T>", "= OtherRemoteName<T>")
    change(peer, provider.as_uri(), 2, texts[provider])
    assert published(peer, provider.as_uri(), 2) == []
    definition(
        "::ImportedAlias", "ImportedAlias", provider, "class OtherRemoteName", "OtherRemoteName"
    )
    assert references(path, "as ImportedAlias", "ImportedAlias", 4) == imported
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    texts[provider] = library
    definition("::ImportedAlias", "ImportedAlias", provider, "class RemoteName", "RemoteName")

    texts[path] = source.replace("= LocalName<T>", "= OtherLocalName<T>")
    change(peer, path.as_uri(), 2, texts[path])
    assert published(peer, path.as_uri(), 2) == []
    definition("::LocalAlias", "LocalAlias", path, "struct OtherLocalName", "OtherLocalName")
    references(path, "struct LocalName", "LocalName", 2)
    references(path, "typealias LocalAlias", "LocalAlias", 4)
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    texts[path] = source
    definition("::LocalAlias", "LocalAlias", path, "struct LocalName", "LocalName")
