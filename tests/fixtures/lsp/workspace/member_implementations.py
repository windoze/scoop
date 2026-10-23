"""Member implementations retain actual overrides, defaults and property origins."""


def check(peer, root):
    provider = root / "dependency/src/memberImplementations.scoop"
    library = (
        "package member_impl\n"
        "public interface Protocol<T> { public fun choose(input: T): T\n"
        "public val flag: Boolean }\n"
        "public interface Defaults<T> : Protocol<T> { public override fun choose(defaultValue: T): T = defaultValue\n"
        "public override val flag: Boolean get() = true }\n"
        "public abstract class Base<T> public constructor() : Protocol<T> { public abstract override fun choose(baseValue: T): T\n"
        "public abstract override val flag: Boolean }\n"
        "public open class Middle<T> public constructor() : Base<T>() { public open override fun choose(middleValue: T): T = middleValue\n"
        "public open override val flag: Boolean get() = true }\n"
        "public abstract class AbstractStep<T> public constructor() : Middle<T>() { public abstract override fun choose(abstractValue: T): T\n"
        "public abstract override val flag: Boolean }\n"
    )
    path = root / "src/memberImplementations.scoop"
    source = (
        "package member_impl_use\n"
        "import member_impl.Protocol\n"
        "import member_impl.Defaults\n"
        "import member_impl.AbstractStep\n"
        "class Concrete(public override val flag: Boolean) : AbstractStep<Boolean>() { public override fun choose(localValue: Boolean): Boolean = localValue }\n"
        "class UsingDefaults : Defaults<Boolean> {}\n"
        "class Other : Protocol<Boolean> { public override fun choose(otherValue: Boolean): Boolean = otherValue\n"
        "public override val flag: Boolean get() = false }\n"
        "fun callMember(value: Protocol<Boolean>): Boolean = value.choose(value.flag)\n"
        "fun boundMember(value: Protocol<Boolean>): (Boolean) -> Boolean = value::choose\n"
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

    def query(file, text, line, name):
        return peer.request(
            "textDocument/implementation",
            {
                "textDocument": {"uri": file.as_uri()},
                "position": {"line": line, "character": text.splitlines()[line].index(name)},
            },
        )

    def location(file, text, line, name):
        start = text.splitlines()[line].index(name)
        return {
            "uri": file.as_uri(),
            "range": {
                "start": {"line": line, "character": start},
                "end": {"line": line, "character": start + len(name)},
            },
        }

    def verify(result, wanted):
        assert result is not None, wanted
        assert len(result) == len(wanted), (result, wanted)
        assert all(item in result for item in wanted), (result, wanted)

    methods = [
        location(provider, library, 3, "choose"),
        location(provider, library, 7, "choose"),
        location(path, source, 4, "choose"),
        location(path, source, 6, "choose"),
    ]
    properties = [
        location(provider, library, 4, "flag"),
        location(provider, library, 8, "flag"),
        location(path, source, 4, "flag"),
        location(path, source, 7, "flag"),
    ]
    verify(query(provider, library, 1, "choose"), methods)
    verify(query(provider, library, 2, "flag"), properties)
    verify(query(path, source, 8, "choose"), methods)
    verify(query(path, source, 9, "choose"), methods)
    verify(query(path, source, 8, "flag"), properties)
    verify(query(provider, library, 5, "choose"), methods[1:3])
    verify(query(provider, library, 7, "choose"), methods[1:3])
    verify(query(provider, library, 9, "choose"), [methods[2]])
    verify(query(provider, library, 10, "flag"), [properties[2]])
    verify(query(path, source, 4, "choose"), [methods[2]])
    verify(query(path, source, 4, "flag"), [properties[2]])

    updated = source.replace("class Concrete(", "abstract class Concrete(").replace(
        "public override fun choose(localValue: Boolean): Boolean = localValue", ""
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": path.as_uri(), "version": 4},
            "contentChanges": [{"text": updated}],
        },
    )
    verify(query(provider, library, 1, "choose"), [item for item in methods if item != methods[2]])
    assert query(provider, library, 9, "choose") == []
    peer.notify("textDocument/didClose", {"textDocument": {"uri": path.as_uri()}})
    verify(query(provider, library, 1, "choose"), methods)

    updated_library = library.replace(
        "choose(defaultValue: T): T = defaultValue", "choose(defaultValue: T): T"
    )
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": provider.as_uri(), "version": 5},
            "contentChanges": [{"text": updated_library}],
        },
    )
    verify(query(path, source, 8, "choose"), methods[1:])
    peer.notify("textDocument/didClose", {"textDocument": {"uri": provider.as_uri()}})
    verify(query(path, source, 8, "choose"), methods)
    assert not (provider.parent.parent / "target").exists()
