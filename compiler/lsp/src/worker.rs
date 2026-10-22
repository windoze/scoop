use std::sync::mpsc::{self, Sender};

use lsp_server::{ErrorCode, RequestId, ResponseError};
use lsp_types::{Diagnostic, Uri};
use scoop_analysis::{
    AnalysisSnapshot, Cancellation, Revision, SemanticSession, SourceId, SourceInput,
};
use serde_json::Value;

use crate::{
    convert,
    initialize::Settings,
    queries::{self, DocumentQuery, QueryKind},
    server::Event,
};

pub(crate) enum WorkKind {
    Request {
        id: RequestId,
        source: SourceId,
        query: DocumentQuery,
        version: Option<i32>,
        cancellation: Cancellation,
    },
    Diagnostics {
        source: SourceId,
        uri: Uri,
        version: i32,
    },
    WorkspaceSymbols {
        id: RequestId,
        query: String,
        cancellation: Cancellation,
    },
}

pub(crate) struct Work {
    pub snapshot: AnalysisSnapshot,
    pub settings: Settings,
    pub kind: WorkKind,
}

pub(crate) enum Completed {
    Request {
        id: RequestId,
        version: Option<i32>,
        result: Result<Value, ResponseError>,
    },
    Diagnostics {
        uri: Uri,
        version: i32,
        result: Result<Vec<Diagnostic>, ResponseError>,
    },
}

pub(crate) struct Completion {
    pub revision: Revision,
    pub value: Completed,
}

pub(crate) fn start(events: Sender<Event>) -> std::io::Result<Sender<Work>> {
    let (send, receive) = mpsc::channel::<Work>();
    std::thread::Builder::new()
        .name("scoop-analysis".to_owned())
        .spawn(move || {
            let mut semantics = SemanticSession::default();
            while let Ok(work) = receive.recv() {
                let revision = work.snapshot.revision;
                let completed = run(work, &mut semantics);
                if events
                    .send(Event::Completed(Completion {
                        revision,
                        value: completed,
                    }))
                    .is_err()
                {
                    break;
                }
            }
        })?;
    Ok(send)
}

fn run(work: Work, semantics: &mut SemanticSession) -> Completed {
    let check_revision = || {
        work.snapshot
            .check_cancelled()
            .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))
    };
    match work.kind {
        WorkKind::Request {
            id,
            source: source_id,
            query,
            version,
            cancellation,
        } => {
            let result = source_input(&work.snapshot, source_id).and_then(|source| {
                cancellation
                    .check()
                    .map_err(|_| convert::error(ErrorCode::RequestCanceled, "request cancelled"))?;
                let result = match query.kind {
                    QueryKind::CodeAction(query_kind) => queries::code_actions::actions(
                        &query.uri,
                        source,
                        version,
                        &query_kind,
                        work.settings,
                        &cancellation,
                    ),
                    QueryKind::ResolveCodeAction(data) => {
                        queries::code_actions::resolve(data, source, version, work.settings)
                    }
                    QueryKind::Syntax(query_kind) => {
                        query_kind.run(&query.uri, source, work.settings, &cancellation)
                    }
                    QueryKind::Hover(position) => queries::hover(
                        &work.snapshot,
                        source_id,
                        position,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                    QueryKind::Definition(position) => queries::definition(
                        &work.snapshot,
                        source_id,
                        position,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                    QueryKind::References {
                        position,
                        include_declaration,
                    } => queries::references(
                        &work.snapshot,
                        source_id,
                        position,
                        include_declaration,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                    QueryKind::Completion(position) => queries::completion(
                        &work.snapshot,
                        source_id,
                        position,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                    QueryKind::SignatureHelp(position) => queries::signature_help(
                        &work.snapshot,
                        source_id,
                        position,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                }?;
                check_revision()?;
                cancellation
                    .check()
                    .map_err(|_| convert::error(ErrorCode::RequestCanceled, "request cancelled"))?;
                Ok(result)
            });
            Completed::Request {
                id,
                version,
                result,
            }
        }
        WorkKind::Diagnostics {
            source: source_id,
            uri,
            version,
        } => {
            let result = source_input(&work.snapshot, source_id).and_then(|source| {
                let diagnostics = crate::diagnostics::collect(
                    &work.snapshot,
                    source_id,
                    source,
                    &uri,
                    work.settings,
                    semantics,
                )?;
                check_revision()?;
                Ok(diagnostics)
            });
            Completed::Diagnostics {
                uri,
                version,
                result,
            }
        }
        WorkKind::WorkspaceSymbols {
            id,
            query,
            cancellation,
        } => {
            let result = (|| {
                cancellation.check().map_err(|_| queries::cancelled())?;
                let result = queries::workspace_symbols(
                    &work.snapshot,
                    &query,
                    work.settings,
                    &cancellation,
                )?;
                check_revision()?;
                cancellation.check().map_err(|_| queries::cancelled())?;
                Ok(result)
            })();
            Completed::Request {
                id,
                version: None,
                result,
            }
        }
    }
}

fn source_input(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
) -> Result<&SourceInput, ResponseError> {
    snapshot
        .source(source)
        .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))?
        .map(|source| source.as_ref())
        .ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source is unavailable"))
}
