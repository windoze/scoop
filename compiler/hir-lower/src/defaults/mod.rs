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
mod scopes;
pub(crate) use scopes::{DefaultScopeRoot, PendingDefaultLocalScope, finish_default_local_scopes};
pub(crate) type DefaultExpressionKey = (
    hir::PersistentLexicalRootV1,
    scoop_identity::StructuralDefinitionPath,
);
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

#[derive(Clone)]
enum InheritedDefaultSource {
    Local(LocalDefaultExprId),
    Export {
        expression: hir::ExportDefaultExprId,
        type_arguments: Vec<hir::TypeId>,
    },
    Imported(std::sync::Arc<hir::ExportDefaultTemplateV1>),
}

#[derive(Clone)]
pub(crate) enum DefaultOverrideSource {
    Local {
        function: hir::FunctionId,
        type_arguments: Vec<hir::TypeId>,
    },
    Imported(scoop_identity::CallableTemplateOrigin),
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
