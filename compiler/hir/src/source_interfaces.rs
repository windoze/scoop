//! Export-side source-call protocol for defaults, named arguments and varargs.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExportValueParameter {
    pub name: String,
    pub calling: ExportParameterCalling,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterCalling {
    Required {
        value_type: TypeId,
    },
    Default {
        value_type: TypeId,
        source: ExportDefaultSourceId,
    },
    Vararg {
        parameter_type: ExportVarargParameterTypeId,
        omission: ExportVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportVarargOmission {
    EmptyArray,
    Default(ExportDefaultSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportVarargParameterType {
    pub element_type: TypeId,
    pub array_type: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterOwner {
    Function(FunctionId),
    StructConstructor(StructId),
    ClassConstructor(ClassId),
    VariantConstructor { enumeration: EnumId, variant: u32 },
}

#[derive(Debug, Clone)]
pub struct ExportParameterInterface {
    pub owner: ExportParameterOwner,
    pub parameters: Vec<ExportValueParameter>,
}

/// A default is a declaration-bound typed body. Parameter references are
/// locals in this template arena and are related to declaration positions by
/// `value_parameters`; callers never resolve their source names again.
#[derive(Debug, Clone)]
pub struct ExportDefaultExpr {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
    pub result_type: TypeId,
    /// The exact declaration identities referenced by `Type::Param` nodes in
    /// the template. An inherited source relates these to its own static view
    /// through `ExportDefaultSource::type_arguments`.
    pub type_parameters: Vec<TypeParamId>,
    pub receiver: Option<ExportDefaultReceiver>,
    pub value_parameters: Vec<ExportDefaultValueParameter>,
    pub origin: DefinitionOrigin,
}

/// A typed inheritance/application edge for one default source. The argument
/// at each position corresponds to the template parameter at the same
/// position and is expressed in the consuming declaration's type scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSource {
    pub expression: ExportDefaultExprId,
    pub type_arguments: Vec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultReceiver {
    pub local: LocalId,
    pub ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultValueParameter {
    pub position: u32,
    pub local: LocalId,
}
