//! Typed declaration views consumed by call resolution.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::{
    CallableCandidate, CallableCandidateOwner, CallableCandidateSource, Lowerer,
    defaults::SourceParameterOwner,
};

mod arrays;
mod parameters;
pub(crate) use parameters::{
    DeclarationSignature, ValueParameter, ValueParameterCalling, VarargOmission,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgumentMode {
    Mixed,
    NamedOnly,
    PositionalOnly,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CallableEffects {
    pub(crate) is_suspend: bool,
    pub(crate) attributes: hir::FunctionAttributes,
}

pub(crate) type SourceDispatch = CallableCandidateSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NominalConstructorSource {
    Struct(hir::StructConstructorId),
    Class(hir::ClassConstructorId),
    IntrinsicClass(hir::ClassId),
    ArrayGenerate(hir::ClassId),
    ImportedArray(hir::SourceNominalId),
    ImportedArrayGenerate(hir::SourceNominalId),
    Variant(hir::EnumVariantRef),
}

/// Constructor and variant declarations expose the same inference surface as
/// callables, but own only host parameters and produce a complete nominal
/// application instead of a callable return type.
#[derive(Debug, Clone)]
pub(crate) struct NominalConstructorView {
    pub(crate) target: NominalConstructorSource,
    pub(crate) signature: DeclarationSignature,
    pub(crate) argument_mode: ArgumentMode,
}

/// Complete declaration-side information consumed by call resolution. It is
/// deliberately a view rather than a new global identity: the target keeps
/// the existing kind-specific id, while all information needed by the solver
/// is explicit and no longer recovered from a name or arena traversal.
#[derive(Debug, Clone)]
pub(crate) struct CallableView {
    pub(crate) target: CallableSource,
    pub(crate) receiver: ReceiverShape,
    pub(crate) signature: DeclarationSignature,
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
        let source_owner = SourceParameterOwner::Function(function);
        CallableView {
            target,
            signature: crate::call_resolution::candidates::DeclarationSignature {
                owner_parameters: signature.type_params[..owner_count].to_vec(),
                callable_parameters: signature.type_params[owner_count..].to_vec(),
                value_parameters: signature
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| ValueParameter {
                        name: parameter.name.text.clone(),
                        calling: self.source_parameter_calling(
                            source_owner,
                            index,
                            parameter.ty,
                            &parameter.calling,
                        ),
                        ty: parameter.ty,
                    })
                    .collect(),
                return_type: signature.return_ty,
            },
            receiver,
            effects: CallableEffects {
                is_suspend: signature.is_suspend,
                attributes: signature.attributes,
            },
            dispatch: candidate.source,
            declaration_span: self.functions[function].span,
        }
    }

    pub(crate) fn nominal_constructor_view(
        &mut self,
        target: NominalConstructorSource,
        span: Span,
    ) -> NominalConstructorView {
        match target {
            NominalConstructorSource::Struct(constructor_id) => {
                let constructor = &self.struct_constructors[constructor_id];
                let structure = constructor.owner;
                let declaration = &self.structs[structure];
                let source_owner = SourceParameterOwner::StructConstructor(constructor_id);
                let calling = self
                    .struct_parameter_calling
                    .get(&constructor_id)
                    .cloned()
                    .unwrap_or_default();
                NominalConstructorView {
                    target,
                    signature: crate::call_resolution::candidates::DeclarationSignature {
                        owner_parameters: declaration.type_params.clone(),
                        callable_parameters: Vec::new(),
                        value_parameters: constructor
                            .parameters
                            .iter()
                            .zip(&calling)
                            .enumerate()
                            .map(|(index, (parameter, calling))| ValueParameter {
                                name: parameter.name.clone(),
                                calling: self.source_parameter_calling(
                                    source_owner,
                                    index,
                                    parameter.ty,
                                    calling,
                                ),
                                ty: parameter.ty,
                            })
                            .collect(),
                        return_type: self.struct_applications[declaration.self_application]
                            .canonical_type,
                    },
                    argument_mode: ArgumentMode::Mixed,
                }
            }
            NominalConstructorSource::Class(constructor_id) => {
                let constructor = &self.class_constructors[constructor_id];
                let class = constructor.owner;
                let declaration = &self.classes[class];
                let source_owner = SourceParameterOwner::ClassConstructor(constructor_id);
                let calling = self
                    .class_parameter_calling
                    .get(&constructor_id)
                    .cloned()
                    .unwrap_or_default();
                NominalConstructorView {
                    target,
                    signature: crate::call_resolution::candidates::DeclarationSignature {
                        owner_parameters: declaration.type_params.clone(),
                        callable_parameters: Vec::new(),
                        value_parameters: constructor
                            .parameters
                            .iter()
                            .zip(&calling)
                            .enumerate()
                            .map(|(index, (parameter, calling))| ValueParameter {
                                name: parameter.name.clone(),
                                calling: self.source_parameter_calling(
                                    source_owner,
                                    index,
                                    parameter.ty,
                                    calling,
                                ),
                                ty: parameter.ty,
                            })
                            .collect(),
                        return_type: self.class_applications[declaration.self_application]
                            .canonical_type,
                    },
                    argument_mode: ArgumentMode::Mixed,
                }
            }
            NominalConstructorSource::IntrinsicClass(class)
            | NominalConstructorSource::ArrayGenerate(class) => {
                let declaration = &self.classes[class];
                self.array_constructor_view(
                    target,
                    declaration.type_params.clone(),
                    self.class_applications[declaration.self_application].canonical_type,
                )
            }
            NominalConstructorSource::ImportedArray(owner)
            | NominalConstructorSource::ImportedArrayGenerate(owner) => {
                self.imported_array_constructor_view(target, owner, span)
            }
            NominalConstructorSource::Variant(variant) => {
                let enumeration = variant.enumeration();
                let variant_index = variant.local_index();
                let declaration = &self.enums[enumeration];
                let source_owner = SourceParameterOwner::VariantConstructor(variant);
                let calling = &self.variant_parameter_calling[&(enumeration, variant_index)];
                let argument_mode = match declaration.variants[variant_index as usize].style {
                    crate::VariantStyle::Unit | crate::VariantStyle::Constructor => {
                        ArgumentMode::Mixed
                    }
                    crate::VariantStyle::Positional => ArgumentMode::PositionalOnly,
                    crate::VariantStyle::Named => ArgumentMode::NamedOnly,
                };
                NominalConstructorView {
                    target,
                    signature: crate::call_resolution::candidates::DeclarationSignature {
                        owner_parameters: declaration.type_params.clone(),
                        callable_parameters: Vec::new(),
                        value_parameters: declaration.variants[variant_index as usize]
                            .fields
                            .iter()
                            .zip(calling)
                            .enumerate()
                            .map(|(index, (field, calling))| ValueParameter {
                                name: field.name.clone(),
                                calling: self.source_parameter_calling(
                                    source_owner,
                                    index,
                                    field.ty,
                                    calling,
                                ),
                                ty: field.ty,
                            })
                            .collect(),
                        return_type: self.enum_applications[declaration.self_application]
                            .canonical_type,
                    },
                    argument_mode,
                }
            }
        }
    }
}
