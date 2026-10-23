"""Rename validates ordinary bindings on unpublished, versioned source edits."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-project"
    manifest(project, "rename")
    api = "src/api.scoop"
    use = "src/use.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            api: "package renamecases\nfun selected(value: Boolean): Boolean = value\n",
            use: (
                "package renamecases\nfun caller(): Boolean = selected(true)\n"
                "fun factoryCall(): Boolean { val factory: (Boolean) -> Boolean = ::selected; return factory(true) }\n"
            ),
        },
    )
    case.clean()
    prepared = case.query(api, "fun selected", "selected", "prepareRename")
    start = case.position(api, "fun selected", "selected")
    assert prepared == {
        "range": {"start": start, "end": {**start, "character": start["character"] + 8}},
        "placeholder": "selected",
    }, prepared
    result = case.query(api, "fun selected", "selected", "rename", newName="chosen")
    symbols = peer.request(
        "textDocument/documentSymbol",
        {"textDocument": {"uri": case.paths[api].as_uri()}},
    )
    assert any(symbol["name"] == "selected" for symbol in symbols), symbols
    case.apply(result, 3)
    assert case.query(api, "fun chosen", "chosen", "rename", newName="chosen") == {
        "documentChanges": []
    }
    references = case.query(
        api, "fun chosen", "chosen", "references", context={"includeDeclaration": True}
    )
    assert len(references) == 3, references

    case.change(
        use,
        case.texts[use]
        + "fun capture(flag: Boolean): Boolean { val original = true; if (flag) { val taken = false; return original }; return original }\n",
    )
    case.clean()
    failure = case.query(use, "val original", "original", "rename", error=-32803, newName="taken")
    assert "binding" in failure["message"], failure
    case.rename(use, "val original", "original", "名字", 3)
    case.rename(use, "val 名字", "名字", "fun", 3)
    prepared = case.query(use, "val `fun`", "`fun`", "prepareRename")
    assert prepared["placeholder"] == "fun", prepared
    case.query(use, "val `fun`", "`fun`", "rename", error=-32602, newName="bad`name")
    case.query(api, "value: Boolean", "Boolean", "prepareRename", error=-32803)
    case.finish()
