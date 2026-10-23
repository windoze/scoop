mod support;

use lsp_server::ErrorCode;
use serde_json::json;
use support::Client;

#[test]
fn rename_advertises_prepare_with_versioned_edits_without_code_action_support() {
    let mut client = Client::start();
    let result = client
        .request(
            "initialize",
            json!({
                "processId": null,
                "capabilities": {"workspace": {"workspaceEdit": {"documentChanges": true}}}
            }),
        )
        .response_result
        .unwrap();
    assert_eq!(
        result["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );
    assert!(result["capabilities"]["codeActionProvider"].is_null());
    client.notify("initialized", json!({}));
    let uri = "untitled:rename.scoop";
    client.open(uri, "fun selected() {}");
    assert!(
        client
            .request(
                "textDocument/prepareRename",
                json!({
                    "textDocument": {"uri": uri}, "position": {"line": 0, "character": 4}
                })
            )
            .response_result
            .unwrap()
            .is_null()
    );
    client.finish();
}

#[test]
fn clients_without_document_changes_do_not_receive_rename_edits() {
    let mut client = Client::start();
    let result = client.initialize("utf-16", false);
    assert!(result["capabilities"]["renameProvider"].is_null());
    let uri = "untitled:rename.scoop";
    client.open(uri, "fun selected() {}");
    for method in ["textDocument/prepareRename", "textDocument/rename"] {
        let error = client.request(method, json!({
            "textDocument": {"uri": uri}, "position": {"line": 0, "character": 4}, "newName": "renamed"
        })).response_result.unwrap_err();
        assert_eq!(error.code, ErrorCode::MethodNotFound as i32);
    }
    client.finish();
}
