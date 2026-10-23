//! Query envelopes adapt wire values to the frontend's ordinary semantic API.

use crate::{AnnotationTarget, CallableDeclaration, MethodId, QueryJson, Reference, TypeReference};
use scoop_hir as hir;
use scoop_identity::{ConeIdentity, PersistentAnnotationId};
use scoop_source::{Diagnostic, DiagnosticSeverity, Span};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod annotations;
mod bodies;
mod defaults;
mod exports;
mod request;
pub use request::{CompilerQuery, DefaultReference, ExportNamespace, Query, TypeRelation};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// The caller has already mapped the request's MethodId to this fixed context.
pub fn execute_query(
    queries: &mut dyn hir::SemanticQueries,
    function: hir::FunctionId,
    query: Query,
) -> Result<Value> {
    let context = hir::SemanticContext::Function(function);
    match query {
        Query::DescribeTypes { types } => {
            let values = types
                .into_iter()
                .map(|reference| {
                    let ty = resolve_type(queries, context, reference)?;
                    crate::type_description(queries, function, ty).map_err(one)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"types": values}))
        }
        Query::DescribeMembers { types } => {
            let values = types
                .into_iter()
                .map(|reference| {
                    let ty = resolve_type(queries, context, reference)?;
                    crate::member_description(queries, function, ty).map_err(one)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"members": values}))
        }
        Query::DescribeAnnotations {
            targets,
            declarations,
        } => annotations::describe(queries, targets, declarations),
        Query::DescribeDefaults { defaults } => {
            let values = defaults
                .into_iter()
                .map(|reference| defaults::describe(queries, context, reference))
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"defaults": values}))
        }
        Query::DescribeBodies { callables } => {
            let values = callables
                .into_iter()
                .map(|reference| bodies::describe(queries, reference))
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"bodies": values}))
        }
        Query::TypeRelation {
            relation,
            left,
            right,
        } => {
            let left = resolve_type(queries, context, left)?;
            let right = resolve_type(queries, context, right)?;
            let relation = match relation {
                TypeRelation::Equal => hir::SemanticTypeRelation::Equal,
                TypeRelation::Subtype => hir::SemanticTypeRelation::Subtype,
            };
            Ok(json!({"holds": queries.type_relation(context, relation, left, right)}))
        }
        Query::LookupExport {
            provider,
            path,
            namespace,
        } => exports::lookup(queries, context, provider, path, namespace),
        Query::SourceLocation { targets } => {
            let values = targets
                .into_iter()
                .map(|target| {
                    let declaration = queries.resolve_declaration(target.into()).map_err(one)?;
                    match declaration {
                        Some(declaration) => encode(QueryJson(&declaration.origin)),
                        None => Ok(Value::Null),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"locations": values}))
        }
    }
}

fn resolve_type(
    queries: &mut dyn hir::SemanticQueries,
    context: hir::SemanticContext,
    reference: TypeReference,
) -> Result<hir::TypeId> {
    queries.resolve_type_reference(context.into(), reference.0, Span::new(0, 0))
}

fn encode(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value)
        .map_err(|cause| error(format!("cannot encode compiler query result: {cause}")))
}

fn one(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    vec![diagnostic]
}

fn error(message: impl Into<String>) -> Vec<Diagnostic> {
    one(Diagnostic::without_span(
        DiagnosticSeverity::Error,
        0,
        message,
    ))
}
