mod support;

use serde_json::json;
use support::Client;

#[test]
fn a_document_without_a_semantic_project_does_not_guess_types() {
    let mut client = Client::start();
    client.initialize("utf-16", true);
    let uri = "untitled:declarations.scoop";
    client.open(uri, "fun name(value: Missing): Boolean { value. }");
    let response = client.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": uri}, "position": {"line": 0, "character": 5},
        }),
    );
    assert!(response.response_result.unwrap().is_null());
    let definition = client.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 5}}),
    );
    assert!(definition.response_result.unwrap().is_null());
    let signature = client.request(
        "textDocument/signatureHelp",
        json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 10}}),
    );
    assert!(signature.response_result.unwrap().is_null());
    assert_eq!(client.symbols(uri)[0]["name"], "name");
    client.finish();
}
