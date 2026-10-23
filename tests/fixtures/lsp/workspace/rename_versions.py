"""Rename responses use current document versions, closed sources and cancellation."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-versions"
    manifest(project, "rename-versions")
    api = "src/api.scoop"
    use = "src/use.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            api: "fun selected(value: Boolean): Boolean = value\n"
            + "".join(f"fun extra{index}(): Boolean = true\n" for index in range(128)),
            use: "fun use(): Boolean = selected(true)\n",
        },
    )
    case.clean()
    params = {
        "textDocument": {"uri": case.paths[api].as_uri()},
        "position": case.position(api, "fun selected", "selected"),
        "newName": "chosen",
    }
    pending = peer.send_request("textDocument/rename", params)
    case.change(use, case.texts[use])
    result = peer.response(pending)
    versions = {
        document["textDocument"]["uri"]: document["textDocument"]["version"]
        for document in result["documentChanges"]
    }
    assert versions == {case.paths[api].as_uri(): 1, case.paths[use].as_uri(): 2}, result
    case.close(use)
    case.rename(api, "fun selected", "selected", "chosen", 2)
    assert "chosen(true)" in case.paths[use].read_text()

    params["position"] = case.position(api, "fun chosen", "chosen")
    params["newName"] = "latest"
    cancelled = peer.send_request("textDocument/rename", params)
    peer.notify("$/cancelRequest", {"id": cancelled})
    peer.response(cancelled, error_code=-32800)
    case.clean()
    case.rename(api, "fun chosen", "chosen", "latest", 2)
    assert "latest(true)" in case.paths[use].read_text()
    case.finish()
