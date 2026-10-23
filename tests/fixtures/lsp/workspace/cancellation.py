"""Request cancellation and edits during real project semantic work."""

from diagnostics import change, published


def check(peer, root):
    path = root / "src/changing.scoop"
    latest = "fun latest(value: Boolean): Boolean = value\n"
    path.write_text(latest)
    uri = path.as_uri()
    text = "".join(f"fun queued{index}(value: Boolean): Boolean = value\n" for index in range(1024))
    peer.notify(
        "textDocument/didOpen",
        {
            "textDocument": {
                "uri": uri,
                "version": 1,
                "languageId": "scoop",
                "text": text,
            }
        },
    )
    cancelled = peer.send_request(
        "textDocument/hover",
        {"textDocument": {"uri": uri}, "position": {"line": 512, "character": 5}},
    )
    peer.notify("$/cancelRequest", {"id": cancelled})
    change(peer, uri, 2, latest)
    peer.response(cancelled, error_code=-32800)
    assert published(peer, uri, 2) == []
    first_current = peer.diagnostic_versions[uri].index(2)
    result = peer.request(
        "textDocument/hover",
        {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 5}},
    )
    assert result["contents"]["value"] == "fun latest(value: Boolean): Boolean", result
    assert all(version == 2 for version in peer.diagnostic_versions[uri][first_current:])
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
