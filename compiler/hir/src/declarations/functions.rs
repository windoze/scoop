//! Callable declarations, effects, generic identity, and native boundaries.

use super::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Complete declaration identity. Generic functions carry their distinct
    /// template id directly; consumers never recover it by scanning the
    /// `generic_functions` arena or by inspecting `type_params`.
    pub genericity: FunctionGenericity,
    /// Whether calls use the coroutine ABI rather than the ordinary ABI.
    pub is_suspend: bool,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    /// Member metadata; the receiver of a method is the first entry of
    /// `params` (named `this`). Top-level functions have `None`.
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionGenericity {
    Plain,
    Generic {
        definition: GenericFunctionId,
        parameters: Vec<TypeParamDecl>,
    },
    /// An ordinary method whose signature/body depends only on its nominal
    /// owner's parameters. Its concrete identity comes from a
    /// `MethodApplication`, not a generic-function application.
    OwnerParameterizedMethod {
        owner_parameters: Vec<TypeParamDecl>,
        no_gc_type_params: Vec<TypeParamId>,
    },
    /// A non-virtual method with its own parameters. The two groups are
    /// structurally separate; no downstream consumer receives a merged
    /// argument vector and an index at which it is expected to split it.
    GenericMethod {
        definition: GenericMethodId,
        owner_parameters: Vec<TypeParamDecl>,
        method_parameters: NonEmptyVec<TypeParamDecl>,
    },
}

impl Function {
    pub fn type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::Plain => 0,
            FunctionGenericity::Generic { parameters, .. } => parameters.len(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters.len() + method_parameters.len(),
        }
    }

    pub fn owner_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            }
            | FunctionGenericity::GenericMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::Plain | FunctionGenericity::Generic { .. } => 0,
        }
    }

    pub fn method_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::GenericMethod {
                method_parameters, ..
            } => method_parameters.len(),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => 0,
        }
    }

    pub fn type_param(&self, id: TypeParamId) -> &TypeParamDecl {
        match &self.genericity {
            FunctionGenericity::Plain => panic!("plain function has no type parameters"),
            FunctionGenericity::Generic { parameters, .. } => parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("generic function owns the referenced type parameter"),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("method owner owns the referenced type parameter"),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .find(|parameter| parameter.id == id)
                .expect("generic method owns the referenced type parameter"),
        }
    }

    pub fn type_params(&self) -> Vec<&TypeParamDecl> {
        match &self.genericity {
            FunctionGenericity::Plain => Vec::new(),
            FunctionGenericity::Generic { parameters, .. } => parameters.iter().collect(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.iter().collect(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .collect(),
        }
    }

    pub fn generic_definition(&self) -> Option<GenericFunctionId> {
        match &self.genericity {
            FunctionGenericity::Plain
            | FunctionGenericity::OwnerParameterizedMethod { .. }
            | FunctionGenericity::GenericMethod { .. } => None,
            FunctionGenericity::Generic { definition, .. } => Some(*definition),
        }
    }

    pub fn generic_method_definition(&self) -> Option<GenericMethodId> {
        match self.genericity {
            FunctionGenericity::GenericMethod { definition, .. } => Some(definition),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAttributes {
    pub safety: Safety,
    pub gc_effect: GcEffect,
    /// M12 currently supports only cdecl for native-addressable functions.
    pub calling_convention: CallingConvention,
}

impl Default for FunctionAttributes {
    fn default() -> Self {
        Self {
            safety: Safety::Safe,
            gc_effect: GcEffect::Managed,
            calling_convention: CallingConvention::Cdecl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
}

#[derive(Debug, Clone)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub safety: Safety,
    pub params: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeId,
    /// Parameters are immutable locals.
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    /// Stable source identity for a conditional derived equality method. Its
    /// application-specific ordinary body lives on
    /// `DerivedEqualityApplication` and must be present before concretization
    /// requests this function.
    DerivedEquality,
    /// A validated compiler intrinsic. Raw annotation text does not cross the
    /// AST/HIR boundary: kind and defining provider are both typed and
    /// mandatory.
    Intrinsic(IntrinsicFunction),
    /// A bodyless native declaration. Complete ABI metadata lives in the
    /// independent extern arena and is referenced by a typed id.
    Extern(ExternFunctionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicFunction {
    pub kind: IntrinsicFunctionKind,
    pub provider: IntrinsicProviderId,
}
