use super::*;
use lsp_server::{ErrorCode, Message, Request, RequestId, Response, ResponseError};
use scoop_generator_protocol::{CompilerQuery, GeneratorDiagnostic, execute_query};
use scoop_process::CommandExt;
use serde_json::json;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn generate(
    queries: &mut dyn hir::SemanticQueries,
    batch: &GeneratorBatch,
    program: &Path,
) -> Result<Generated, Diagnostic> {
    let origin = &batch
        .tasks
        .first()
        .expect("planned batch")
        .methods
        .first()
        .origin;
    let failure = |message: String| {
        diagnostic(
            origin,
            format!("generator `{}` failed: {message}", batch.generator),
        )
    };
    let params = batch.request(queries)?;
    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .scoop_spawn()
        .map_err(|error| failure(format!("cannot start {}: {error}", program.display())))?;
    let mut input = BufWriter::new(child.stdin.take().expect("piped stdin"));
    let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
    let id = RequestId::from("generate-1".to_owned());
    Message::Request(Request {
        id: id.clone(),
        method: "generator/generate".into(),
        params,
    })
    .write(&mut input)
    .map_err(|error| failure(error.to_string()))?;
    let methods = batch
        .tasks
        .iter()
        .flat_map(|task| task.methods.iter())
        .map(|method| (method.method, method))
        .collect::<HashMap<_, _>>();
    let result = loop {
        let message = Message::read(&mut output)
            .map_err(|error| failure(error.to_string()))?
            .ok_or_else(|| failure("protocol output ended before the generated result".into()))?;
        match message {
            Message::Request(request) => {
                let response = respond(queries, &methods, request);
                Message::Response(response)
                    .write(&mut input)
                    .map_err(|error| failure(error.to_string()))?;
            }
            Message::Response(response) if response.id == id => {
                let value = response.response_result.map_err(|error| {
                    failure(format!("JSON-RPC error {}: {}", error.code, error.message))
                })?;
                break serde_json::from_value(value)
                    .map_err(|error| failure(format!("invalid generated result: {error}")))?;
            }
            Message::Response(_) => {
                return Err(failure(
                    "response id does not match generator/generate".into(),
                ));
            }
            Message::Notification(_) => {
                return Err(failure(
                    "notifications are not part of the generator protocol".into(),
                ));
            }
        }
    };
    drop(input);
    if !output
        .fill_buf()
        .map_err(|error| failure(error.to_string()))?
        .is_empty()
    {
        return Err(failure(
            "extra protocol output follows the final result".into(),
        ));
    }
    let status = child.wait().map_err(|error| failure(error.to_string()))?;
    if !status.success() {
        return Err(failure(format!("process exited with {status}")));
    }
    Ok(result)
}

fn respond(
    queries: &mut dyn hir::SemanticQueries,
    methods: &HashMap<MethodId, &GeneratorMethod>,
    request: Request,
) -> Response {
    let result = (|| {
        if !serde_json::to_value(&request.id)
            .expect("request ids are serializable")
            .is_string()
        {
            return Err(error(
                ErrorCode::InvalidRequest,
                "generator request ids must be strings",
            ));
        }
        if request.method != "compiler/query" {
            return Err(error(
                ErrorCode::MethodNotFound,
                "unknown generator protocol method",
            ));
        }
        let query: CompilerQuery = serde_json::from_value(request.params)
            .map_err(|detail| error(ErrorCode::InvalidParams, detail.to_string()))?;
        let method = methods.get(&query.context).ok_or_else(|| {
            error(
                ErrorCode::InvalidParams,
                "query context is not a method in this generator batch",
            )
        })?;
        execute_query(queries, method.pending.function, query.query).map_err(|diagnostics| {
            let data = diagnostics
                .iter()
                .map(|diagnostic| GeneratorDiagnostic::from_diagnostic(diagnostic, &method.origin))
                .collect::<Vec<_>>();
            ResponseError {
                code: scoop_generator_protocol::QUERY_ERROR,
                message: "compiler query failed".into(),
                data: Some(json!({"diagnostics": data})),
            }
        })
    })();
    Response {
        id: request.id,
        response_result: result,
    }
}

fn error(code: ErrorCode, message: impl Into<String>) -> ResponseError {
    ResponseError {
        code: code as i32,
        message: message.into(),
        data: None,
    }
}
