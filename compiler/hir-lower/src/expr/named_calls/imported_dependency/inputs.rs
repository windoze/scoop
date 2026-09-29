use super::ImportedArgumentMap;
use crate::Lowerer;
use crate::call_resolution::arguments::ArgumentShapeFailure;
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
    pub(super) type_args: &'a [ast::CallTypeArgument],
    pub(in crate::expr) arguments: ImportedCallArguments<'a>,
    pub(in crate::expr) span: ast::Span,
}

impl<'a> From<CallSite<'a>> for ImportedProbeCall<'a> {
    fn from(call: CallSite<'a>) -> Self {
        Self {
            type_args: call.type_args,
            arguments: ImportedCallArguments::Source(call.args),
            span: call.span,
        }
    }
}

impl<'a> ImportedProbeCall<'a> {
    pub(in crate::expr) fn lowered(arguments: &'a [hir::Expr], span: ast::Span) -> Self {
        Self {
            type_args: &[],
            arguments: ImportedCallArguments::Lowered(arguments),
            span,
        }
    }
}

impl<'a> ImportedCallArguments<'a> {
    pub(super) fn source(self, index: usize) -> Option<&'a ast::Expr> {
        match self {
            Self::Source(arguments) => Some(&arguments[index].expression),
            Self::Lowered(_) => None,
        }
    }

    pub(super) fn check_variant_style(
        self,
        style: hir::EnumSourceVariantStyleV1,
    ) -> Result<(), ArgumentShapeFailure> {
        let (positional, named) = match self {
            Self::Source(arguments) => (
                arguments
                    .iter()
                    .any(|argument| matches!(argument.name, ast::CallArgumentName::Positional)),
                arguments
                    .iter()
                    .any(|argument| matches!(argument.name, ast::CallArgumentName::Named(_))),
            ),
            Self::Lowered(arguments) => (!arguments.is_empty(), false),
        };
        match style {
            hir::EnumSourceVariantStyleV1::Named if positional => {
                Err(ArgumentShapeFailure::PositionalForNamedOnly)
            }
            hir::EnumSourceVariantStyleV1::Positional if named => {
                Err(ArgumentShapeFailure::NamedForPositionalOnly)
            }
            _ => Ok(()),
        }
    }

    pub(super) fn len(self) -> usize {
        match self {
            Self::Source(arguments) => arguments.len(),
            Self::Lowered(arguments) => arguments.len(),
        }
    }

    pub(super) fn map(
        self,
        parameters: &[hir::CallableSourceParameterV1],
        operator_set: bool,
    ) -> Result<ImportedArgumentMap, ArgumentShapeFailure> {
        match self {
            Self::Source(arguments) if operator_set => {
                ImportedArgumentMap::source_operator_set(parameters, arguments)
            }
            Self::Source(arguments) => ImportedArgumentMap::source(parameters, arguments),
            Self::Lowered(arguments) => ImportedArgumentMap::lowered(parameters, arguments.len()),
        }
    }

    pub(super) fn lower(
        self,
        index: usize,
        state: &mut Lowerer,
        sink: &mut Vec<hir::Statement>,
        expected: Option<hir::TypeId>,
    ) -> Option<hir::Expr> {
        match self {
            Self::Source(arguments) => {
                state.lower_expr(&arguments[index].expression, sink, expected)
            }
            Self::Lowered(arguments) => Some(arguments[index].clone()),
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
