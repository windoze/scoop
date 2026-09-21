//! Definition-site lowering of exported source-parameter interfaces.

use la_arena::Idx;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{FnParamCalling, Lowerer, Owner};

mod access;
mod calling;
mod inherited;
mod instantiate;
mod lowering;
mod preparation;
mod registration;
pub(crate) use preparation::{DefaultArgumentSource, DefaultPreparation, SourceDefaultKey};

#[derive(Clone)]
pub(crate) struct LocalDefaultExpr {
    pub(crate) body: hir::ExportDefaultExpr,
    pub(crate) captures: Vec<hir::Capture>,
}

pub(crate) type LocalDefaultExprId = Idx<LocalDefaultExpr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultExprTemplateRef {
    Local(LocalDefaultExprId),
    Export(hir::ExportDefaultSourceId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InheritedDefaultSource {
    Local(LocalDefaultExprId),
    Export {
        expression: hir::ExportDefaultExprId,
        type_arguments: Vec<hir::TypeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceParameterOwner {
    Function(hir::FunctionId),
    StructConstructor(hir::StructConstructorId),
    ClassConstructor(hir::ClassConstructorId),
    VariantConstructor(hir::EnumVariantRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceParameterCalling {
    Required,
    Default(DefaultArgumentSource),
    Vararg {
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        omission: SourceVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceVarargOmission {
    EmptyArray,
    Default(DefaultArgumentSource),
}

#[derive(Clone)]
struct ParameterSource {
    name: ast::Ident,
    ty: hir::TypeId,
    calling: FnParamCalling,
}

#[derive(Clone)]
struct DefaultContext {
    definition_root: hir::LexicalDefinitionRoot,
    source_context: hir::SourceContextSubject,
    type_parameters: Vec<hir::TypeParamDecl>,
    receiver: Option<(hir::TypeId, Option<Owner>)>,
    is_suspend: bool,
    safety: hir::Safety,
    callable_name: String,
}
