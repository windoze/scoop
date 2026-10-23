//! Existing typed bodies and defaults, without execution or body generation.

use crate::*;

/// A callable already known to the current frontend or dependency view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticCallable {
    Current(FunctionId),
    CurrentGetter(PropertyGetterId),
    CurrentSetter(PropertySetterId),
    Imported(DefaultCallableDeclarationV1),
}

/// Availability is distinct from failure to resolve a declaration reference.
#[derive(Clone, Debug)]
pub enum SemanticBodyAvailability<T> {
    Available(T),
    PendingGeneration,
    NotPublished,
    NoBody,
}

#[derive(Clone, Debug)]
pub enum SemanticBody<'a> {
    Current(&'a Body),
    Imported(ImportedCallableBody),
}

/// Definition-side data is retained in its original binder and local scope.
#[derive(Clone, Copy, Debug)]
pub enum SemanticDefaultDefinition<'a> {
    Current {
        expression: ExportDefaultExprId,
        template: &'a ExportDefaultExpr,
    },
    Imported(&'a ExportDefaultTemplateV1),
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticDefaultSource<'a> {
    pub source: ExportDefaultSourceId,
    pub definition: SemanticDefaultDefinition<'a>,
    /// Arguments in the consuming declaration's scope, including inheritance.
    pub type_arguments: &'a [TypeId],
}
