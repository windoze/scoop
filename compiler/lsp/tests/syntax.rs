mod support;

use serde_json::json;
use support::Client;

#[test]
fn erroneous_documents_offer_structure_and_clear_repaired_diagnostics() {
    let mut client = Client::start();
    client.initialize("utf-16", true);
    let uri = "untitled:syntax.scoop";
    client.open(
        uri,
        "fun broken(value: Int) {\r\n    value.\r\n}\r\nfun 正常(): Int = 1\r\n",
    );
    assert!(
        !client
            .diagnostics(uri, Some(1))
            .as_array()
            .unwrap()
            .is_empty()
    );
    let symbols = client.symbols(uri);
    assert_eq!(symbols[0]["name"], "broken");
    assert_eq!(symbols[1]["name"], "正常");
    assert_eq!(
        symbols[1]["selectionRange"]["start"],
        json!({"line": 3, "character": 4})
    );
    let folds = client
        .request(
            "textDocument/foldingRange",
            json!({"textDocument": {"uri": uri}}),
        )
        .response_result
        .unwrap();
    assert!(
        folds
            .as_array()
            .unwrap()
            .iter()
            .any(|range| range["startLine"] == 0 && range["endLine"] == 2)
    );
    let selection = client.request("textDocument/selectionRange", json!({"textDocument": {"uri": uri}, "positions": [{"line": 3, "character": 5}, {"line": 4, "character": 0}]})).response_result.unwrap();
    assert_eq!(
        selection[0]["range"],
        json!({"start": {"line": 3, "character": 4}, "end": {"line": 3, "character": 6}})
    );
    assert_eq!(
        selection[1]["range"]["end"],
        json!({"line": 4, "character": 0})
    );
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"range": {"start": {"line": 1, "character": 9}, "end": {"line": 1, "character": 10}}, "text": ""}]}));
    assert!(
        client
            .diagnostics(uri, Some(2))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.finish();
}

#[test]
fn flat_symbols_preserve_containers_and_nested_functions_are_functions() {
    let mut client = Client::start();
    client.initialize("utf-16", false);
    let uri = "untitled:flat.scoop";
    client.open(uri, "class C { fun method() { fun local() = 1 } }");
    let symbols = client.symbols(uri);
    assert_eq!(symbols[0]["name"], "C");
    assert_eq!(symbols[1]["containerName"], "C");
    assert_eq!(symbols[1]["kind"], 6);
    assert_eq!(symbols[2]["containerName"], "method");
    assert_eq!(symbols[2]["kind"], 12);
    client.finish();
}

#[test]
fn conditional_interface_blocks_keep_the_host_symbol_and_offer_folding() {
    let mut client = Client::start();
    client.initialize("utf-16", true);
    let uri = "untitled:conditional.scoop";
    client.open(
        uri,
        "interface Flag { fun enabled(): Boolean }\n\
         class Holder<T> {\n\
             override interface Flag where T : ref {\n\
                 public override fun enabled(): Boolean = true\n\
             }\n\
         }\n",
    );
    assert!(
        client
            .diagnostics(uri, Some(1))
            .as_array()
            .unwrap()
            .is_empty()
    );
    let symbols = client.symbols(uri);
    assert_eq!(symbols.as_array().unwrap().len(), 2);
    assert_eq!(symbols[1]["name"], "Holder");
    let members = symbols[1]["children"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["name"], "enabled");
    assert_eq!(members[0]["kind"], 6);
    let folds = client
        .request(
            "textDocument/foldingRange",
            json!({"textDocument": {"uri": uri}}),
        )
        .response_result
        .unwrap();
    assert!(
        folds
            .as_array()
            .unwrap()
            .iter()
            .any(|fold| { fold["startLine"] == 2 && fold["endLine"] == 4 })
    );
    client.finish();
}
