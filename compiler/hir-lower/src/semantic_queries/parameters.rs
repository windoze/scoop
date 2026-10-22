//! Source parameter views keep default providers and vararg semantics intact.

use super::*;
use crate::imported_core::ImportedTypeBindings;
use scoop_identity::CallableTemplateOrigin;

mod imported;

impl Lowerer {
    pub(super) fn query_signature_bindings(
        &mut self,
        owner_count: usize,
        parameters: &[hir::TypeParamDecl],
    ) -> ImportedTypeBindings {
        let shape = hir::DefaultTemplateProviderShapeV1::try_new(
            u32::try_from(owner_count).expect("owner arity fits the identity schema"),
            u32::try_from(parameters.len() - owner_count)
                .expect("callable arity fits the identity schema"),
        )
        .expect("checked declaration binders fit the signature schema");
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                (
                    shape
                        .identity_binder_at(index as u32)
                        .expect("this is an actual declaration parameter"),
                    self.intern_type(hir::Type::Param(parameter.id)),
                )
            })
            .collect()
    }

    pub(super) fn query_current_parameters(
        &mut self,
        owner: hir::ExportParameterOwner,
        identity: CallableTemplateOrigin,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Vec<hir::SemanticParameterDescription> {
        let parameters = self
            .source_parameter_interfaces
            .iter()
            .find(|interface| interface.owner == owner)
            .expect("a checked source callable retains its complete parameter interface")
            .parameters
            .clone();
        let callable = match owner {
            hir::ExportParameterOwner::Function(id) => hir::SourceAnnotationCallable::Function(id),
            hir::ExportParameterOwner::StructConstructor(id) => {
                hir::SourceAnnotationCallable::StructConstructor(id)
            }
            hir::ExportParameterOwner::ClassConstructor(id) => {
                hir::SourceAnnotationCallable::ClassConstructor(id)
            }
            hir::ExportParameterOwner::VariantConstructor(id) => {
                hir::SourceAnnotationCallable::VariantConstructor(id)
            }
        };
        parameters
            .into_iter()
            .enumerate()
            .map(|(index, parameter)| {
                let (ty, calling) = match parameter.calling {
                    hir::ExportParameterCalling::Required { value_type } => {
                        (value_type, hir::SemanticParameterCalling::Required)
                    }
                    hir::ExportParameterCalling::Default { value_type, source } => (
                        value_type,
                        hir::SemanticParameterCalling::Default(
                            self.query_current_default(source, substitutions),
                        ),
                    ),
                    hir::ExportParameterCalling::Vararg {
                        parameter_type,
                        omission,
                    } => {
                        let ty = self.export_vararg_parameter_types[parameter_type];
                        (
                            ty.array_type,
                            hir::SemanticParameterCalling::Vararg {
                                element_type: self.query_type_use(ty.element_type, substitutions),
                                omission: match omission {
                                    hir::ExportVarargOmission::EmptyArray => {
                                        hir::SemanticVarargOmission::EmptyArray
                                    }
                                    hir::ExportVarargOmission::Default(source) => {
                                        hir::SemanticVarargOmission::Default(
                                            self.query_current_default(source, substitutions),
                                        )
                                    }
                                },
                            },
                        )
                    }
                };
                let index = u32::try_from(index).expect("source parameter arity fits u32");
                hir::SemanticParameterDescription {
                    identity: hir::SemanticParameterIdentity {
                        owner: identity,
                        index,
                    },
                    name: parameter.name,
                    value_type: self.query_type_use(ty, substitutions),
                    calling,
                    target: hir::StaticParameterTarget::Plain,
                    annotations: hir::SemanticAnnotationTarget::Current(
                        hir::SourceAnnotationTarget::Parameter { callable, index },
                    ),
                    origin: hir::SemanticSourceLocation {
                        source: self.source_identity(parameter.origin.file as usize).clone(),
                        span: parameter.origin.span,
                    },
                }
            })
            .collect()
    }

    fn query_current_default(
        &mut self,
        source: hir::ExportDefaultSourceId,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> hir::SemanticDefaultUse {
        let arguments = self.export_default_sources[source]
            .type_arguments()
            .to_vec();
        hir::SemanticDefaultUse {
            definition: hir::SemanticDefaultReference::Current(source),
            declaration_arguments: arguments.clone(),
            arguments: arguments
                .into_iter()
                .map(|ty| self.instantiate_method_ty(ty, substitutions))
                .collect(),
        }
    }
}
