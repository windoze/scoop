"""Constructor signatures share source parameters and preserve variant forms."""

from diagnostics import change, published


def check(peer, root):
    provider = root / "dependency/src/constructor_hovers.scoop"
    library = (
        "package constructorhoverapi\n"
        "public class RemoteHover<T> public constructor(value: T, enabled: Boolean = true)\n"
        "public class RemoteChoices {\n"
        "    /// Builds the remote Boolean choice.\n"
        "    /// @param value The remote choice input.\n"
        "    public constructor(value: Boolean, enabled: Boolean = true) {}\n"
        "    public constructor(value: String) {}\n"
        "}\n"
        "public enum HoverEvent<T> {\n"
        "    /// The empty case.\n"
        "    Empty,\n"
        "    /// The tuple case.\n"
        "    Tuple(T),\n"
        "    /// The named case.\n"
        "    Named { item: T },\n"
        "    /// The constructor case.\n"
        "    /// @param value The case input.\n"
        "    Item(val value: T, val enabled: Boolean = true)\n"
        "}\n"
    )
    path = root / "src/constructor_hovers.scoop"
    source = (
        "import constructorhoverapi.RemoteHover as SelectedHover\n"
        "import constructorhoverapi.RemoteChoices\n"
        "import constructorhoverapi.HoverEvent\n"
        "import constructorhoverapi.HoverEvent.Item as BuildHover\n"
        "import constructorhoverapi.HoverEvent.Empty as EmptyHover\n"
        "class LocalHover<T>(value: T, enabled: Boolean = true)\n"
        "struct StructHover(val value: Boolean)\n"
        "class LocalChoices {\n"
        "    /// Builds the local Boolean choice.\n"
        "    /// @param value The local choice input.\n"
        "    constructor(value: Boolean, enabled: Boolean = true) {}\n"
        "    constructor(value: String) {}\n"
        "}\n"
        "enum LocalHoverEvent<T> { Empty, Tuple(T), Named { item: T }, Item(val value: T) }\n"
        "fun constructorHovers() {\n"
        "    val local = /* 🌟 */ LocalHover(value = true)\n"
        "    val structure = StructHover(value = true)\n"
        "    val first = LocalChoices(value = true)\n"
        '    val second = LocalChoices(value = "text")\n'
        "    val remote = SelectedHover(value = true)\n"
        "    val choice = RemoteChoices(value = true)\n"
        "    val factory: (Boolean, Boolean) -> SelectedHover<Boolean> = ::SelectedHover\n"
        "    val unit: HoverEvent<Boolean> = EmptyHover\n"
        "    val tuple = HoverEvent.Tuple(true)\n"
        "    val named = HoverEvent.Named(item = true)\n"
        "    val item = BuildHover(value = true)\n"
        "    val localTuple = LocalHoverEvent.Tuple(true)\n"
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

    def position(text, needle, name):
        prefix = text[: text.index(needle) + needle.index(name)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(needle, name, method="hover"):
        return peer.request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": position(source, needle, name),
            },
        )

    def hover(needle, name, label, documentation=None):
        result = query(needle, name)
        contents = result["contents"]
        if documentation is None:
            assert contents == {"language": "scoop", "value": label}, result
        else:
            assert contents == [{"language": "scoop", "value": label}, documentation], result
        start = position(source, needle, name)
        assert result["range"] == {
            "start": start,
            "end": {**start, "character": start["character"] + len(name)},
        }, result

    def definition(needle, name, file, text, declaration, target):
        result = query(needle, name, "definition")
        start = position(text, declaration, target)
        assert result == {
            "uri": file.as_uri(),
            "range": {
                "start": start,
                "end": {**start, "character": start["character"] + len(target)},
            },
        }, result

    assert published(peer, path.as_uri(), 1) == []
    hover(
        "LocalHover(value",
        "LocalHover",
        "constructor LocalHover<T>(value: T, enabled: Boolean = true)",
    )
    hover("StructHover(value", "StructHover", "constructor StructHover(value: Boolean)")
    local_label = "constructor LocalChoices(value: Boolean, enabled: Boolean = true)"
    hover(
        "LocalChoices(value = true)",
        "LocalChoices",
        local_label,
        "Builds the local Boolean choice.\n\n**@param** value — The local choice input.",
    )
    hover('LocalChoices(value = "text")', "LocalChoices", "constructor LocalChoices(value: String)")
    for needle in ["SelectedHover(value", "::SelectedHover"]:
        hover(
            needle, "SelectedHover", "constructor RemoteHover<T>(value: T, enabled: Boolean = true)"
        )
    remote_docs = "Builds the remote Boolean choice.\n\n**@param** value — The remote choice input."
    hover(
        "RemoteChoices(value",
        "RemoteChoices",
        "constructor RemoteChoices(value: Boolean, enabled: Boolean = true)",
        remote_docs,
    )
    for needle, name, label, docs in [
        ("Empty as EmptyHover", "Empty", "HoverEvent<T>.Empty", "The empty case."),
        ("HoverEvent.Tuple(true)", "Tuple", "HoverEvent<T>.Tuple(T)", "The tuple case."),
        ("HoverEvent.Named(item", "Named", "HoverEvent<T>.Named { item: T }", "The named case."),
        (
            "BuildHover(value",
            "BuildHover",
            "HoverEvent<T>.Item(value: T, enabled: Boolean = true)",
            "The constructor case.\n\n**@param** value — The case input.",
        ),
        ("LocalHoverEvent.Tuple(true)", "Tuple", "LocalHoverEvent<T>.Tuple(T)", None),
        ("Empty, Tuple(T)", "Empty", "LocalHoverEvent<T>.Empty", None),
    ]:
        hover(needle, name, label, docs)
    hover(
        "constructor(value: Boolean",
        "constructor",
        local_label,
        "Builds the local Boolean choice.\n\n**@param** value — The local choice input.",
    )
    hover("constructor(value: Boolean", "value", "value: Boolean", "The local choice input.")
    hover("LocalHover<T>(value", "value", "value: T")
    hover("LocalHover<T>(value: T, enabled", "enabled", "enabled: Boolean = true")
    definition(
        "constructor(value: Boolean",
        "constructor",
        path,
        source,
        "constructor(value: Boolean",
        "constructor",
    )
    definition("LocalHover<T>(value", "value", path, source, "LocalHover<T>(value", "value")
    definition(
        "RemoteChoices(value",
        "RemoteChoices",
        provider,
        library,
        "constructor(value: Boolean",
        "constructor",
    )
    definition("BuildHover(value", "BuildHover", provider, library, "Item(val value", "Item")

    original_library = library
    library = library.replace("enabled: Boolean = true", "enabled: Boolean = false").replace(
        "The tuple case.", "The updated tuple case."
    )
    change(peer, provider.as_uri(), 2, library)
    assert published(peer, provider.as_uri(), 2) == []
    hover(
        "SelectedHover(value",
        "SelectedHover",
        "constructor RemoteHover<T>(value: T, enabled: Boolean = false)",
    )
    hover("HoverEvent.Tuple(true)", "Tuple", "HoverEvent<T>.Tuple(T)", "The updated tuple case.")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    library = original_library
    hover(
        "SelectedHover(value",
        "SelectedHover",
        "constructor RemoteHover<T>(value: T, enabled: Boolean = true)",
    )

    original_source = source
    source += "class UncalledHover { constructor(enabled: Boolean = missing) {} }\nfun brokenHover() { missing }\n"
    change(peer, path.as_uri(), 2, source)
    assert published(peer, path.as_uri(), 2)
    hover(
        "UncalledHover { constructor",
        "constructor",
        "constructor UncalledHover(enabled: Boolean = missing)",
    )
    hover("UncalledHover { constructor(enabled", "enabled", "enabled: Boolean = missing")
    hover(
        "LocalHover(value",
        "LocalHover",
        "constructor LocalHover<T>(value: T, enabled: Boolean = true)",
    )
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    source = original_source
    hover(
        "LocalHover(value",
        "LocalHover",
        "constructor LocalHover<T>(value: T, enabled: Boolean = true)",
    )
