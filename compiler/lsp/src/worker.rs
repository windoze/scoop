use std::sync::mpsc::{self, Sender};

use lsp_server::{ErrorCode, RequestId, ResponseError};
use lsp_types::{Diagnostic, Uri};
use scoop_analysis::{AnalysisSnapshot, Cancellation, Revision, SemanticSession, SourceId};
use serde_json::Value;

use crate::{
    convert,
    initialize::Settings,
    queries::{self, Query, QueryKind},
    server::Event,
};

pub(crate) enum WorkKind {
    Request {
        id: RequestId,
        query: Query,
        cancellation: Cancellation,
    },
    Diagnostics {
        uri: Uri,
        version: i32,
    },
}

pub(crate) struct Work {
    pub source: SourceId,
    pub snapshot: AnalysisSnapshot,
    pub settings: Settings,
    pub kind: WorkKind,
}

pub(crate) enum Completed {
    Request {
        id: RequestId,
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
    let input = work
        .snapshot
        .source(work.source)
        .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))
        .and_then(|source| {
            source.ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source is unavailable"))
        });
    let check_revision = || {
        work.snapshot
            .check_cancelled()
            .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))
    };
    match work.kind {
        WorkKind::Request {
            id,
            query,
            cancellation,
        } => {
            let result = input.and_then(|source| {
                cancellation
                    .check()
                    .map_err(|_| convert::error(ErrorCode::RequestCanceled, "request cancelled"))?;
                let result = match query.kind {
                    QueryKind::Syntax(query_kind) => {
                        query_kind.run(&query.uri, source, work.settings, &cancellation)
                    }
                    QueryKind::Hover(position) => queries::hover(
                        &work.snapshot,
                        work.source,
                        position,
                        work.settings,
                        semantics,
                        &cancellation,
                    ),
                    QueryKind::Definition(position) => queries::definition(
                        &work.snapshot,
                        work.source,
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
            Completed::Request { id, result }
        }
        WorkKind::Diagnostics { uri, version } => {
            let result = input.and_then(|source| {
                let parsed = source
                    .parse()
                    .map_err(|err| convert::error(ErrorCode::RequestFailed, err))?;
                let mut diagnostics = Vec::new();
                for diagnostic in parsed.diagnostics() {
                    if let Some(diagnostic) =
                        convert::diagnostic(&source.text, &uri, diagnostic, work.settings.encoding)?
                    {
                        diagnostics.push(diagnostic);
                    }
                }
                check_revision()?;
                Ok(diagnostics)
            });
            Completed::Diagnostics {
                uri,
                version,
                result,
            }
        }
    }
}
