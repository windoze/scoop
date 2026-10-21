mod support;

use lsp_server::ErrorCode;
use serde_json::json;
use support::Client;

#[test]
fn cancel_is_received_during_parsing_and_new_input_wins() {
    let mut client = Client::start();
    client.initialize("utf-16", true);
    let uri = "untitled:changing.scoop";
    let source: String = (0..5000)
        .map(|index| format!("fun function{index}(value: Int) = value\n"))
        .collect();
    client.open(uri, &source);
    let cancelled = client.send_request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": uri}}),
    );
    client.notify("$/cancelRequest", json!({"id": cancelled}));
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": "fun latest() = 1"}]}));
    assert_eq!(
        client.response(cancelled).response_result.unwrap_err().code,
        ErrorCode::RequestCanceled as i32
    );
    let symbols = client.symbols(uri);
    assert_eq!(symbols.as_array().unwrap().len(), 1);
    assert_eq!(symbols[0]["name"], "latest");
    assert!(
        client
            .diagnostics(uri, Some(2))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notifications.clear();
    assert_eq!(client.symbols(uri)[0]["name"], "latest");
    assert!(
        !client
            .notifications
            .iter()
            .any(
                |notification| notification.method == "textDocument/publishDiagnostics"
                    && notification.params["version"] == 1
            )
    );
    client.finish();
}
