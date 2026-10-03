//! Declaration parameter types and source calling protocols share one view.

use crate::defaults::DefaultArgumentSource;
use scoop_hir as hir;

#[derive(Debug, Clone)]
pub(crate) struct DeclarationSignature<D = DefaultArgumentSource> {
    pub(crate) owner_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) callable_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) value_parameters: Vec<ValueParameter<D>>,
    pub(crate) return_type: hir::TypeId,
}

#[derive(Debug, Clone)]
pub(crate) struct ValueParameter<D = DefaultArgumentSource> {
    pub(crate) name: String,
    pub(crate) calling: ValueParameterCalling<D>,
    pub(crate) ty: hir::TypeId,
}

impl<D> ValueParameter<D> {
    pub(crate) fn is_vararg(&self) -> bool {
        matches!(self.calling, ValueParameterCalling::Vararg { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueParameterCalling<D = DefaultArgumentSource> {
    Required,
    Default(D),
    Vararg {
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        omission: VarargOmission<D>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarargOmission<D = DefaultArgumentSource> {
    EmptyArray,
    Default(D),
}
