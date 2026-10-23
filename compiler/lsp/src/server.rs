use std::{
    collections::HashMap,
    io::{self, Write},
    sync::mpsc::{self, Sender},
};

use lsp_server::{ErrorCode, Message, Request, RequestId, Response};
use lsp_types::Uri;
use scoop_analysis::{Cancellation, Revision};

use crate::{
    convert,
    documents::Documents,
    initialize::Settings,
    queries::{self, Query},
    worker::{self, Completion, Work, WorkKind},
};

mod notifications;
mod results;
mod workspace;

pub(crate) enum Event {
    Incoming(Message),
    Closed(Option<String>),
    Completed(Completion),
}

struct Pending {
    query: Query,
    cancellation: Cancellation,
}

struct Server {
    documents: Documents,
    workspace_settings: crate::workspace::WorkspaceSettings,
    workspace_loader: scoop_analysis::WorkspaceLoader,
    project_diagnostics: HashMap<Uri, (Option<i32>, Vec<lsp_types::Diagnostic>)>,
    settings: Settings,
    initialized: bool,
    shutdown: bool,
    pending: HashMap<RequestId, Pending>,
    diagnostics: HashMap<Uri, (Revision, i32)>,
    worker: Sender<Work>,
}

pub fn run_stdio() -> io::Result<i32> {
    let (events, incoming) = mpsc::channel();
    let transport = events.clone();
    std::thread::Builder::new()
        .name("scoop-lsp-input".to_owned())
        .spawn(move || {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            loop {
                let event = match Message::read(&mut input) {
                    Ok(Some(message)) => Event::Incoming(message),
                    Ok(None) => {
                        let _ = transport.send(Event::Closed(None));
                        break;
                    }
                    Err(error) => {
                        let _ = transport.send(Event::Closed(Some(error.to_string())));
                        break;
                    }
                };
                if transport.send(event).is_err() {
                    break;
                }
            }
        })?;
    let worker = worker::start(events)?;
    let mut server = Server {
        documents: Documents::default(),
        workspace_settings: Default::default(),
        workspace_loader: Default::default(),
        project_diagnostics: HashMap::new(),
        settings: Settings::default(),
        initialized: false,
        shutdown: false,
        pending: HashMap::new(),
        diagnostics: HashMap::new(),
        worker,
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();
    while let Ok(event) = incoming.recv() {
        match event {
            Event::Incoming(Message::Request(request)) => server.request(request, &mut output)?,
            Event::Incoming(Message::Notification(notification))
                if notification.method == "exit" =>
            {
                server.cancel_all(&mut output)?;
                return Ok(if server.shutdown { 0 } else { 1 });
            }
            Event::Incoming(Message::Notification(notification)) => {
                server.notification(notification, &mut output)?
            }
            Event::Incoming(Message::Response(response)) => {
                if response.id == RequestId::from("scoop-watch-inputs".to_owned())
                    && let Err(error) = response.response_result
                {
                    Server::log(
                        format!(
                            "cannot register input file notifications: {}",
                            error.message
                        ),
                        &mut output,
                    )?;
                }
            }
            Event::Completed(result) => server.completed(result, &mut output)?,
            Event::Closed(error) => {
                if let Some(error) = error {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, error));
                }
                return Ok(if server.shutdown { 0 } else { 1 });
            }
        }
    }
    Ok(1)
}

impl Server {
    fn request(&mut self, request: Request, output: &mut impl Write) -> io::Result<()> {
        if request.method == "initialize" && !self.initialized && !self.shutdown {
            let result = queries::params(request.params).and_then(|params| {
                self.workspace_settings.initialize(&params);
                let (settings, result) = crate::initialize::initialize(params);
                self.settings = settings;
                self.initialized = true;
                queries::json(result)
            });
            Message::Response(Response {
                id: request.id,
                response_result: result,
            })
            .write(output)?;
            if self.initialized {
                self.refresh_workspace(output)?;
            }
            return Ok(());
        }
        let error = if !self.initialized {
            Some(convert::error(
                ErrorCode::ServerNotInitialized,
                "initialize must be the first request",
            ))
        } else if self.shutdown {
            Some(convert::error(
                ErrorCode::InvalidRequest,
                "server has shut down",
            ))
        } else if request.method == "initialize" {
            Some(convert::error(
                ErrorCode::InvalidRequest,
                "server is already initialized",
            ))
        } else {
            None
        };
        if let Some(error) = error {
            return self.respond_error(request.id, error, output);
        }
        if request.method == "shutdown" {
            self.shutdown = true;
            self.cancel_all(output)?;
            return Message::Response(Response::new_ok(request.id, serde_json::Value::Null))
                .write(output);
        }
        let query = match Query::from_request(&request) {
            Ok(query) => query,
            Err(error) => return self.respond_error(request.id, error, output),
        };
        if let Some(uri) = query.document_uri()
            && let Err(error) = self.documents.load(uri)
        {
            return self.respond_error(request.id, error, output);
        }
        let id = request.id;
        if self.pending.contains_key(&id) {
            return self.respond_error(
                id,
                convert::error(ErrorCode::InvalidRequest, "request id is already active"),
                output,
            );
        }
        self.pending.insert(
            id.clone(),
            Pending {
                query,
                cancellation: Cancellation::default(),
            },
        );
        self.schedule_request(&id, output)
    }

    fn send_work(&self, work: Work) -> io::Result<()> {
        self.worker
            .send(work)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "analysis worker stopped"))
    }

    fn schedule_request(&mut self, id: &RequestId, output: &mut impl Write) -> io::Result<()> {
        let pending = &self.pending[id];
        let kind = match &pending.query {
            Query::WorkspaceSymbols(query) => WorkKind::WorkspaceSymbols {
                id: id.clone(),
                query: query.clone(),
                cancellation: pending.cancellation.clone(),
            },
            Query::Document(query) => {
                let Some(document) = self.documents.get(&query.uri) else {
                    self.pending.remove(id);
                    return self.respond_error(
                        id.clone(),
                        convert::error(ErrorCode::RequestFailed, "document is unavailable"),
                        output,
                    );
                };
                WorkKind::Request {
                    id: id.clone(),
                    source: document.source,
                    query: query.clone(),
                    version: document.version,
                    cancellation: pending.cancellation.clone(),
                }
            }
        };
        self.send_work(Work {
            snapshot: self.documents.host.snapshot(),
            settings: self.settings,
            kind,
        })
    }

    fn schedule_diagnostics(&mut self, uri: &Uri) -> io::Result<()> {
        if self.shutdown {
            return Ok(());
        }
        let Some(document) = self.documents.get(uri) else {
            return Ok(());
        };
        if document
            .path
            .as_ref()
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "Cone.toml"))
        {
            return Ok(());
        }
        let Some(version) = document.version else {
            return Ok(());
        };
        let key = (self.documents.host.revision(), version);
        if self.diagnostics.get(uri) == Some(&key) {
            return Ok(());
        }
        self.diagnostics.insert(uri.clone(), key);
        self.send_work(Work {
            snapshot: self.documents.host.snapshot(),
            settings: self.settings,
            kind: WorkKind::Diagnostics {
                source: document.source,
                uri: uri.clone(),
                version,
            },
        })
    }

    fn cancel_all(&mut self, output: &mut impl Write) -> io::Result<()> {
        for (id, pending) in self.pending.drain() {
            pending.cancellation.cancel();
            Message::Response(Response::new_err(
                id,
                ErrorCode::RequestCanceled as i32,
                "server is exiting".to_owned(),
            ))
            .write(output)?;
        }
        self.diagnostics.clear();
        Ok(())
    }

    fn respond_error(
        &self,
        id: RequestId,
        error: lsp_server::ResponseError,
        output: &mut impl Write,
    ) -> io::Result<()> {
        Message::Response(Response {
            id,
            response_result: Err(error),
        })
        .write(output)
    }
}
