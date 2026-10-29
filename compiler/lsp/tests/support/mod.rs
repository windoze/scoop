#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::BufReader,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use lsp_server::{Message, Notification, Request, RequestId, Response};
use serde_json::{Value, json};

pub struct Client {
    child: Child,
    input: ChildStdin,
    messages: Receiver<Message>,
    responses: HashMap<RequestId, Response>,
    pub notifications: Vec<Notification>,
    pub registrations: HashMap<String, Value>,
    next_id: i32,
    response_timeout: Duration,
}

impl Client {
    pub fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_scoop-lsp"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut output = BufReader::new(stdout);
            while let Ok(Some(message)) = Message::read(&mut output) {
                if send.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            messages,
            responses: HashMap::new(),
            notifications: Vec::new(),
            registrations: HashMap::new(),
            next_id: 1,
            response_timeout: Duration::from_secs(20),
        }
    }

    pub fn set_response_timeout(&mut self, timeout: Duration) {
        self.response_timeout = timeout;
    }

    pub fn initialize(&mut self, encoding: &str, hierarchical: bool) -> Value {
        let result = self.request("initialize", json!({"processId": null, "rootUri": null, "capabilities": {
            "general": {"positionEncodings": [encoding]}, "textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": hierarchical}}
        }})).response_result.unwrap();
        self.notify("initialized", json!({}));
        result
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        Message::Notification(Notification::new(method.to_owned(), params))
            .write(&mut self.input)
            .unwrap();
    }

    pub fn send_request(&mut self, method: &str, params: Value) -> RequestId {
        let id = RequestId::from(self.next_id);
        self.next_id += 1;
        Message::Request(Request::new(id.clone(), method.to_owned(), params))
            .write(&mut self.input)
            .unwrap();
        id
    }

    pub fn request(&mut self, method: &str, params: Value) -> Response {
        let id = self.send_request(method, params);
        self.response(id)
    }

    pub fn response(&mut self, id: RequestId) -> Response {
        loop {
            if let Some(response) = self.responses.remove(&id) {
                return response;
            }
            self.receive();
        }
    }

    fn receive(&mut self) {
        match self
            .messages
            .recv_timeout(self.response_timeout)
            .expect("LSP response timed out")
        {
            Message::Response(response) => {
                self.responses.insert(response.id.clone(), response);
            }
            Message::Notification(notification) => self.notifications.push(notification),
            Message::Request(request) => {
                match request.method.as_str() {
                    "client/registerCapability" => {
                        for registration in request.params["registrations"].as_array().unwrap() {
                            self.registrations.insert(
                                registration["id"].as_str().unwrap().to_owned(),
                                registration.clone(),
                            );
                        }
                    }
                    "client/unregisterCapability" => {
                        for registration in request.params["unregisterations"].as_array().unwrap() {
                            self.registrations
                                .remove(registration["id"].as_str().unwrap());
                        }
                    }
                    _ => panic!("unexpected server request: {request:?}"),
                }
                Message::Response(Response::new_ok(request.id, Value::Null))
                    .write(&mut self.input)
                    .unwrap();
            }
        }
    }

    pub fn diagnostics(&mut self, uri: &str, version: Option<i32>) -> Value {
        loop {
            if let Some(index) = self.notifications.iter().position(|notification| {
                notification.method == "textDocument/publishDiagnostics"
                    && notification.params["uri"] == uri
                    && notification.params.get("version").and_then(Value::as_i64)
                        == version.map(i64::from)
            }) {
                return self.notifications.remove(index).params["diagnostics"].clone();
            }
            self.receive();
        }
    }

    pub fn open(&mut self, uri: &str, text: &str) {
        self.notify("textDocument/didOpen", json!({"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": text}}));
    }

    pub fn symbols(&mut self, uri: &str) -> Value {
        self.request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": uri}}),
        )
        .response_result
        .unwrap()
    }

    pub fn finish(&mut self) {
        assert!(
            self.request("shutdown", Value::Null)
                .response_result
                .unwrap()
                .is_null()
        );
        self.notify("exit", Value::Null);
        self.wait_exit(0);
    }

    pub fn wait_exit(&mut self, code: i32) {
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(code));
                return;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "LSP did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
