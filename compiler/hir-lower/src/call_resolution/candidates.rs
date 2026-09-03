//! Typed, read-only views over callable declarations.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::{CallableCandidate, CallableCandidateOwner, CallableCandidateSource, Lowerer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableSource {
    Free(hir::FunctionId),
    Local {
        local: hir::LocalFunctionId,
        function: hir::FunctionId,
    },
    Method(hir::FunctionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiverShape {
    None,
    Instance,
    Extension(hir::TypeId),
}

#[derive(Debug, Clone)]
pub(crate) struct ValueParameter {
    pub(crate) name: String,
    pub(crate) ty: hir::TypeId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CallableEffects {
    pub(crate) is_suspend: bool,
    pub(crate) attributes: hir::FunctionAttributes,
}

pub(crate) type SourceDispatch = CallableCandidateSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NominalConstructorSource {
    Struct(hir::StructId),
    Class(hir::ClassId),
    Variant {
        enumeration: hir::EnumId,
        variant: u32,
    },
}

/// Constructor and variant declarations expose the same inference surface as
/// callables, but own only host parameters and produce a complete nominal
/// application instead of a callable return type.
#[derive(Debug, Clone)]
pub(crate) struct NominalConstructorView {
    pub(crate) target: NominalConstructorSource,
    pub(crate) owner_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) value_parameters: Vec<ValueParameter>,
    pub(crate) result_type: hir::TypeId,
    pub(crate) declaration_span: Span,
}

/// Complete declaration-side information consumed by call resolution. It is
/// deliberately a view rather than a new global identity: the target keeps
/// the existing kind-specific id, while all information needed by the solver
/// is explicit and no longer recovered from a name or arena traversal.
#[derive(Debug, Clone)]
pub(crate) struct CallableView {
    pub(crate) target: CallableSource,
    pub(crate) receiver: ReceiverShape,
    pub(crate) owner_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) callable_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) value_parameters: Vec<ValueParameter>,
    pub(crate) return_type: hir::TypeId,
    pub(crate) effects: CallableEffects,
    pub(crate) dispatch: SourceDispatch,
    pub(crate) declaration_span: Span,
}

impl CallableView {
    pub(crate) fn function(&self) -> hir::FunctionId {
        match self.target {
            CallableSource::Free(function) | CallableSource::Method(function) => function,
            CallableSource::Local { function, .. } => function,
        }
    }
}

impl Lowerer {
    pub(crate) fn callable_view(
        &self,
        candidate: &CallableCandidate,
        extension: bool,
    ) -> CallableView {
        let function = candidate.function;
        let signature = &self.signatures[&function];
        let target = match candidate.owner {
            CallableCandidateOwner::Method(_) => CallableSource::Method(function),
            CallableCandidateOwner::Function { .. } => self
                .local_function_by_function
                .get(&function)
                .copied()
                .map(|local| CallableSource::Local { local, function })
                .unwrap_or(CallableSource::Free(function)),
        };
        let receiver = if extension {
            ReceiverShape::Extension(
                *self
                    .extension_receivers
                    .get(&function)
                    .expect("extension callable view has a receiver"),
            )
        } else if matches!(candidate.owner, CallableCandidateOwner::Method(_)) {
            ReceiverShape::Instance
        } else {
            ReceiverShape::None
        };
        let owner_count = signature.owner_type_param_count;
        CallableView {
            target,
            receiver,
            owner_parameters: signature.type_params[..owner_count].to_vec(),
            callable_parameters: signature.type_params[owner_count..].to_vec(),
            value_parameters: signature
                .params
                .iter()
                .map(|parameter| ValueParameter {
                    name: parameter.name.text.clone(),
                    ty: parameter.ty,
                })
                .collect(),
            return_type: signature.return_ty,
            effects: CallableEffects {
                is_suspend: signature.is_suspend,
                attributes: signature.attributes,
            },
            dispatch: candidate.source,
            declaration_span: self.functions[function].span,
        }
    }

    pub(crate) fn nominal_constructor_view(
        &self,
        target: NominalConstructorSource,
    ) -> NominalConstructorView {
        match target {
            NominalConstructorSource::Struct(structure) => {
                let declaration = &self.structs[structure];
                NominalConstructorView {
                    target,
                    owner_parameters: declaration.type_params.clone(),
                    value_parameters: declaration
                        .semantic_fields()
                        .iter()
                        .map(|field| ValueParameter {
                            name: field.name.clone(),
                            ty: field.ty,
                        })
                        .collect(),
                    result_type: self.struct_applications[declaration.self_application]
                        .canonical_type,
                    declaration_span: declaration.span,
                }
            }
            NominalConstructorSource::Class(class) => {
                let declaration = &self.classes[class];
                NominalConstructorView {
                    target,
                    owner_parameters: declaration.type_params.clone(),
                    value_parameters: declaration
                        .semantic_constructor()
                        .iter()
                        .map(|property| ValueParameter {
                            name: property.name.clone(),
                            ty: property.ty,
                        })
                        .collect(),
                    result_type: self.class_applications[declaration.self_application]
                        .canonical_type,
                    declaration_span: declaration.span,
                }
            }
            NominalConstructorSource::Variant {
                enumeration,
                variant,
            } => {
                let declaration = &self.enums[enumeration];
                NominalConstructorView {
                    target,
                    owner_parameters: declaration.type_params.clone(),
                    value_parameters: declaration.variants[variant as usize]
                        .fields
                        .iter()
                        .map(|field| ValueParameter {
                            name: field.name.clone(),
                            ty: field.ty,
                        })
                        .collect(),
                    result_type: self.enum_applications[declaration.self_application]
                        .canonical_type,
                    declaration_span: declaration.span,
                }
            }
        }
    }
}
