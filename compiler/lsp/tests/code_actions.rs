mod support;

use lsp_server::ErrorCode;
use serde_json::{Value, json};
use support::Client;

fn initialize(client: &mut Client, resolve: bool) -> Value {
    let result = client.request("initialize", json!({
        "processId": null, "rootUri": null,
        "capabilities": {
            "workspace": {"workspaceEdit": {"documentChanges": true}},
            "textDocument": {"codeAction": {
                "codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix"]}},
                "dataSupport": resolve,
                "resolveSupport": {"properties": if resolve { vec!["edit"] } else { Vec::new() }}
            }}
        }
    })).response_result.unwrap();
    client.notify("initialized", json!({}));
    result
}

fn actions(client: &mut Client, uri: &str, range: Value, only: Option<&str>) -> Value {
    client
        .request(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": uri}, "range": range,
                "context": {"diagnostics": [], "only": only.map(|kind| vec![kind])}
            }),
        )
        .response_result
        .unwrap()
}

fn whole_line() -> Value {
    json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1000}})
}

fn document_edit(action: &Value) -> &Value {
    &action["edit"]["documentChanges"][0]
}

#[test]
fn unicode_syntax_repairs_use_document_versions_and_clear_diagnostics_when_applied() {
    let mut client = Client::start();
    let capabilities = initialize(&mut client, true);
    assert_eq!(
        capabilities["capabilities"]["codeActionProvider"]["resolveProvider"],
        true
    );
    let uri = "untitled:repairs.scoop";
    let text = "fun `😀名`(value: Int other: Int { return value }";
    client.open(uri, text);
    assert_eq!(
        client.diagnostics(uri, Some(1)).as_array().unwrap().len(),
        2
    );
    let fixes = actions(&mut client, uri, whole_line(), Some("quickfix"));
    let fixes = fixes.as_array().unwrap();
    assert_eq!(fixes.len(), 2);
    let mut changes = Vec::new();
    for (fix, insertion) in fixes.iter().zip(["other:", "{"]) {
        let edit = document_edit(fix);
        assert_eq!(edit["textDocument"], json!({"uri": uri, "version": 1}));
        let offset = text[..text.find(insertion).unwrap()].encode_utf16().count();
        let point = json!({"line": 0, "character": offset});
        assert_eq!(
            edit["edits"][0]["range"],
            json!({"start": point, "end": point})
        );
        changes
            .push(json!({"range": edit["edits"][0]["range"], "text": edit["edits"][0]["newText"]}));
    }
    assert!(
        actions(&mut client, uri, whole_line(), Some("refactor"))
            .as_array()
            .unwrap()
            .is_empty()
    );
    let point = document_edit(&fixes[0])["edits"][0]["range"].clone();
    assert_eq!(
        actions(&mut client, uri, point, None)
            .as_array()
            .unwrap()
            .len(),
        1
    );

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 7}, "contentChanges": [{"text": text}]
        }),
    );
    let refreshed = client
        .request("codeAction/resolve", fixes[0].clone())
        .response_result
        .unwrap();
    assert_eq!(document_edit(&refreshed)["textDocument"]["version"], 7);
    changes.reverse();
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 8}, "contentChanges": changes
        }),
    );
    assert!(
        client
            .diagnostics(uri, Some(8))
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        actions(&mut client, uri, whole_line(), None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    let stale = client
        .request("codeAction/resolve", fixes[0].clone())
        .response_result
        .unwrap_err();
    assert_eq!(stale.code, ErrorCode::ContentModified as i32);
    client.finish();
}

#[test]
fn eof_closers_are_concrete_but_missing_values_have_no_quick_fix() {
    let mut client = Client::start();
    let initialized = initialize(&mut client, false);
    assert_eq!(
        initialized["capabilities"]["codeActionProvider"]["resolveProvider"],
        false
    );
    let uri = "untitled:eof.scoop";
    client.open(uri, "fun f() {\r\n");
    let eof = json!({"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}});
    let repairs = actions(&mut client, uri, eof.clone(), None);
    assert_eq!(repairs.as_array().unwrap().len(), 1);
    assert!(repairs[0].get("data").is_none());
    let edit = document_edit(&repairs[0]);
    assert_eq!(edit["edits"][0], json!({"range": eof, "newText": "}"}));
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 2},
            "contentChanges": [{"range": edit["edits"][0]["range"], "text": "}"}]
        }),
    );
    assert!(
        client
            .diagnostics(uri, Some(2))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 3},
            "contentChanges": [{"text": "fun f() { value. }"}]
        }),
    );
    assert!(
        actions(&mut client, uri, whole_line(), None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.finish();
}

#[test]
fn clients_without_versioned_edits_are_not_offered_editing_capabilities() {
    let mut client = Client::start();
    let initialized = client.initialize("utf-16", false);
    assert!(
        initialized["capabilities"]
            .get("codeActionProvider")
            .is_none()
    );
    client.finish();
}
