use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;
use crate::call_resolution::arguments::{CandidateArgumentMap, ParameterInput};
use crate::call_resolution::candidates::NominalConstructorView;

mod declarations;
mod parameters;

#[derive(Clone, Copy)]
pub(crate) enum ArgumentEvaluation {
    Source,
    Lowered,
}

pub(crate) struct CallableArgumentMaterialization<'a> {
    pub function: hir::FunctionId,
    pub argument_map: &'a CandidateArgumentMap,
    pub type_args: &'a [hir::TypeId],
    pub receiver: Option<hir::Expr>,
    pub source_args: Vec<hir::Expr>,
    pub argument_sinks: Vec<Vec<hir::Statement>>,
    pub call_span: Span,
    pub evaluation: ArgumentEvaluation,
}

pub(crate) struct NominalArgumentMaterialization<'a> {
    pub view: &'a NominalConstructorView,
    pub argument_map: &'a CandidateArgumentMap,
    pub type_args: &'a [hir::TypeId],
    pub source_args: Vec<hir::Expr>,
    pub argument_sinks: Vec<Vec<hir::Statement>>,
    pub call_span: Span,
}

pub(crate) struct ResolvedArgumentMaterialization<'a, D> {
    pub parameters: &'a [(String, hir::TypeId)],
    pub inputs: &'a [ParameterInput<D>],
    pub receiver: Option<hir::Expr>,
    pub source_args: Vec<hir::Expr>,
    pub argument_sinks: Vec<Vec<hir::Statement>>,
    pub call_span: Span,
    pub evaluation: ArgumentEvaluation,
    pub temporary_prefix: &'static str,
}

pub(crate) struct DefaultArgumentEvaluation<'a> {
    pub receiver: Option<&'a hir::Expr>,
    pub parameters: &'a [hir::Expr],
    pub call_span: Span,
    pub sink: &'a mut Vec<hir::Statement>,
}

impl Lowerer {
    pub(crate) fn array_assembly(
        &self,
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        parts: Vec<hir::ArrayAssemblyPart>,
        span: Span,
    ) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::ArrayAssembly(hir::ArrayAssembly {
                element_type,
                parts,
                result_type: array_type,
            }),
            ty: array_type,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn materialize_temporary(
        &mut self,
        name: String,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = value.ty;
        let local = self.alloc_synthetic_local(
            name,
            ty,
            false,
            scoop_identity::SyntheticLocalRole::Temporary,
        );
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
