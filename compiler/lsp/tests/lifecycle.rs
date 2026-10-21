mod support;

use lsp_server::ErrorCode;
use serde_json::{Value, json};
use support::Client;

#[test]
fn lifecycle_advertises_only_working_capabilities_and_answers_every_request() {
    let mut client = Client::start();
    let response = client.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": "untitled:test.scoop"}}),
    );
    assert_eq!(
        response.response_result.unwrap_err().code,
        ErrorCode::ServerNotInitialized as i32
    );
    let initialized = client.initialize("utf-8", true);
    let capabilities = &initialized["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-8");
    assert_eq!(capabilities["textDocumentSync"]["change"], 2);
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(capabilities["foldingRangeProvider"], true);
    assert_eq!(capabilities["selectionRangeProvider"], true);
    assert!(capabilities.get("completionProvider").is_none());
    assert_eq!(
        client
            .request("initialize", json!({}))
            .response_result
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest as i32
    );
    assert_eq!(
        client
            .request("unknown/request", Value::Null)
            .response_result
            .unwrap_err()
            .code,
        ErrorCode::MethodNotFound as i32
    );
    assert!(
        client
            .request("shutdown", Value::Null)
            .response_result
            .unwrap()
            .is_null()
    );
    assert_eq!(
        client
            .request("unknown/request", Value::Null)
            .response_result
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest as i32
    );
    client.notify("exit", Value::Null);
    client.wait_exit(0);
}

#[test]
fn exiting_before_shutdown_uses_the_protocol_failure_status() {
    let mut client = Client::start();
    client.notify("exit", Value::Null);
    client.wait_exit(1);
}

#[test]
fn absent_or_unknown_position_encoding_defaults_to_utf16() {
    let mut client = Client::start();
    let initialized = client.initialize("unknown-encoding", false);
    assert_eq!(initialized["capabilities"]["positionEncoding"], "utf-16");
    client.finish();
    let mut client = Client::start();
    let initialized = client
        .request("initialize", json!({"capabilities": {}}))
        .response_result
        .unwrap();
    assert_eq!(initialized["capabilities"]["positionEncoding"], "utf-16");
    client.finish();
}
