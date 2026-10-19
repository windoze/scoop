use super::ImportedArgumentMap;
use crate::call_resolution::arguments::{ArgumentShape, ArgumentShapeFailure};
use crate::call_resolution::candidates::{ArgumentMode, ValueParameter};
use crate::call_resolution::contextual::ArgumentExpression;
use crate::expr::CallSite;
use scoop_ast as ast;
use scoop_hir as hir;

#[derive(Clone, Copy)]
pub(in crate::expr) enum ImportedCallArguments<'a> {
    Source(&'a [ast::CallArgument]),
    Lowered(&'a [hir::Expr]),
}

#[derive(Clone, Copy)]
pub(in crate::expr) struct ImportedProbeCall<'a> {
    pub(in crate::expr) alias: Option<&'a crate::aliases::AliasTemplate>,
    pub(super) type_args: ImportedTypeArguments<'a>,
    pub(in crate::expr) arguments: ImportedCallArguments<'a>,
    pub(in crate::expr) span: ast::Span,
}

impl<'a> From<CallSite<'a>> for ImportedProbeCall<'a> {
    fn from(call: CallSite<'a>) -> Self {
        Self {
            alias: None,
            type_args: ImportedTypeArguments::Source(call.type_args),
            arguments: ImportedCallArguments::Source(call.args),
            span: call.span,
        }
    }
}

impl<'a> ImportedProbeCall<'a> {
    pub(in crate::expr) fn lowered(arguments: &'a [hir::Expr], span: ast::Span) -> Self {
        Self {
            alias: None,
            type_args: ImportedTypeArguments::Source(&[]),
            arguments: ImportedCallArguments::Lowered(arguments),
            span,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ImportedTypeArguments<'a> {
    Source(&'a [ast::CallTypeArgument]),
    Resolved(&'a [crate::expr::ResolvedCallTypeArgument]),
}

impl ImportedTypeArguments<'_> {
    pub(super) fn span(self, index: usize) -> ast::Span {
        match self {
            Self::Source(args) => args[index].span(),
            Self::Resolved(args) => match args[index] {
                crate::expr::ResolvedCallTypeArgument::Explicit { span, .. }
                | crate::expr::ResolvedCallTypeArgument::Infer { span } => span,
            },
        }
    }

    pub(super) fn len(self) -> usize {
        match self {
            Self::Source(args) => args.len(),
            Self::Resolved(args) => args.len(),
        }
    }
    pub(super) fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub(super) fn resolve(
        self,
        state: &mut crate::Lowerer,
    ) -> Option<Vec<crate::expr::ResolvedCallTypeArgument>> {
        match self {
            Self::Source(args) => state.resolve_call_type_args(args),
            Self::Resolved(args) => Some(args.to_vec()),
        }
    }
}

impl<'a> ImportedProbeCall<'a> {
    pub(in crate::expr) fn with_resolved_type_arguments(
        mut self,
        arguments: &'a [crate::expr::ResolvedCallTypeArgument],
    ) -> Self {
        self.type_args = ImportedTypeArguments::Resolved(arguments);
        self
    }
}

impl<'a> ImportedCallArguments<'a> {
    pub(super) fn source(self, index: usize) -> Option<&'a ast::Expr> {
        match self {
            Self::Source(arguments) => Some(&arguments[index].expression),
            Self::Lowered(_) => None,
        }
    }

    pub(super) fn map(
        self,
        parameters: &[ValueParameter<hir::ExportDefaultTemplateKeyV1>],
        mode: ArgumentMode,
        operator_set: bool,
    ) -> Result<ImportedArgumentMap, ArgumentShapeFailure> {
        let arguments = match self {
            Self::Source(arguments) => arguments.iter().map(ArgumentShape::from).collect(),
            Self::Lowered(arguments) => vec![
                ArgumentShape {
                    name: None,
                    spread: false,
                    trailing: false
                };
                arguments.len()
            ],
        };
        ImportedArgumentMap::map(parameters, &arguments, mode, operator_set)
    }

    pub(super) fn expressions(self) -> Vec<ArgumentExpression<'a>> {
        match self {
            Self::Source(arguments) => arguments
                .iter()
                .map(|argument| ArgumentExpression::Source(&argument.expression))
                .collect(),
            Self::Lowered(arguments) => arguments.iter().map(ArgumentExpression::Lowered).collect(),
        }
    }

    pub(super) fn span(self, index: usize) -> ast::Span {
        match self {
            Self::Source(arguments) => arguments[index].span,
            Self::Lowered(arguments) => arguments[index].span,
        }
    }
}

#[derive(Clone)]
pub(in crate::expr) enum ImportedMemberReceiver {
    Value(hir::Expr),
    LiteralSubject(hir::TypeId),
}

impl ImportedMemberReceiver {
    pub(in crate::expr) fn ty(&self) -> hir::TypeId {
        match self {
            Self::Value(value) => value.ty,
            Self::LiteralSubject(ty) => *ty,
        }
    }
}

pub(super) enum ImportedCallReceiver {
    Absent,
    Member {
        value: ImportedMemberReceiver,
        static_type: hir::TypeId,
    },
}
