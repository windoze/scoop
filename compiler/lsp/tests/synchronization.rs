mod support;

use serde_json::json;
use support::Client;

#[test]
fn every_position_encoding_applies_sequential_edits_in_the_current_text() {
    for (encoding, emoji_end, name_end, chinese_width) in [
        ("utf-8", 9, 13, 3),
        ("utf-16", 7, 9, 1),
        ("utf-32", 6, 8, 1),
    ] {
        let mut client = Client::start();
        client.initialize(encoding, true);
        let uri = "untitled:unicode.scoop";
        client.open(uri, "fun `😀名`() = 1\r\n");
        let symbols = client.symbols(uri);
        assert_eq!(symbols[0]["name"], "😀名");
        assert_eq!(symbols[0]["selectionRange"]["end"]["character"], name_end);
        if encoding != "utf-32" {
            client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [
                {"range": {"start": {"line": 0, "character": 6}, "end": {"line": 0, "character": emoji_end}}, "text": "bad"}
            ]}));
            assert_eq!(client.symbols(uri)[0]["name"], "😀名");
        }
        client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [
            {"range": {"start": {"line": 0, "character": 5}, "end": {"line": 0, "character": emoji_end}}, "text": "x"},
            {"range": {"start": {"line": 0, "character": 6}, "end": {"line": 0, "character": 6 + chinese_width}}, "text": "new"}
        ]}));
        assert_eq!(client.symbols(uri)[0]["name"], "xnew");
        assert!(
            client
                .diagnostics(uri, Some(2))
                .as_array()
                .unwrap()
                .is_empty()
        );
        client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": "fun stale() = 1"}]}));
        assert_eq!(client.symbols(uri)[0]["name"], "xnew");
        client.finish();
    }
}

#[test]
fn a_failed_later_edit_does_not_publish_the_first_edit_of_a_notification() {
    let mut client = Client::start();
    client.initialize("utf-16", true);
    let uri = "untitled:atomic.scoop";
    client.open(uri, "fun original() = 1");
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [
        {"text": "fun changed() = 1"},
        {"range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 0}}, "text": "broken"}
    ]}));
    assert_eq!(client.symbols(uri)[0]["name"], "original");
    client.finish();
}

#[test]
fn open_buffers_override_file_events_and_close_reloads_the_actual_disk_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("file with spaces.scoop");
    std::fs::write(&path, "fun disk() = 1").unwrap();
    let uri = url::Url::from_file_path(&path).unwrap().to_string();
    let mut client = Client::start();
    client.initialize("utf-16", true);
    client.open(&uri, "fun buffer() = 2");
    std::fs::write(&path, "fun updatedDisk() = 3").unwrap();
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri, "type": 2}]}),
    );
    assert_eq!(client.symbols(&uri)[0]["name"], "buffer");
    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": uri}}),
    );
    assert!(
        client
            .diagnostics(&uri, None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(client.symbols(&uri)[0]["name"], "updatedDisk");
    client.open(&uri, "fun reopened() = 4");
    assert_eq!(client.symbols(&uri)[0]["name"], "reopened");
    client.finish();
}
