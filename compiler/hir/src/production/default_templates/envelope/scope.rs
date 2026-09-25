//! Provider binder scopes retain host and method parameters in declaration order.
use super::super::{DefaultTemplateEnvelopeProjectionError, entities::DefaultEntityProjector};
use crate::production::signatures::HirInterfaceSignatureProjector;
use crate::{
    ExportHir, FunctionGenericity, HirSignatureBinder, PersistentLexicalRootV1, TypeParamDecl,
    TypeParamId,
};

pub(in crate::production::default_templates) struct ProviderScope {
    pub(in crate::production::default_templates) root: PersistentLexicalRootV1,
    pub(in crate::production::default_templates) binders: Vec<HirSignatureBinder>,
    pub(in crate::production::default_templates) flattened: Vec<TypeParamId>,
}

pub(in crate::production::default_templates) fn provider_scope(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_>,
    root: crate::LexicalDefinitionRoot,
) -> Result<ProviderScope, DefaultTemplateEnvelopeProjectionError> {
    let signatures = HirInterfaceSignatureProjector::new(export);
    let (binders, flattened) = match root {
        crate::LexicalDefinitionRoot::Function(id) => {
            let function = super::super::arena_get(&export.functions, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider function",
                        index: super::super::raw_index(id),
                    },
                ),
            )?;

            let binders = signatures
                .function_binders(function)
                .map_err(DefaultTemplateEnvelopeProjectionError::Signature)?;
            let flattened = flattened_function_parameters(&function.genericity);
            (binders, flattened)
        }
        crate::LexicalDefinitionRoot::StructConstructor(id) => {
            let constructor = super::super::arena_get(&export.struct_constructors, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider struct constructor",
                        index: super::super::raw_index(id),
                    },
                ),
            )?;
            let owner = super::super::arena_get(&export.structs, constructor.owner).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider struct",
                        index: super::super::raw_index(constructor.owner),
                    },
                ),
            )?;

            (
                signatures
                    .binder_frame(&owner.type_params, 0)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Signature)?,
                parameter_ids(&owner.type_params),
            )
        }
        crate::LexicalDefinitionRoot::ClassConstructor(id) => {
            let constructor = super::super::arena_get(&export.class_constructors, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider class constructor",
                        index: super::super::raw_index(id),
                    },
                ),
            )?;
            let owner = super::super::arena_get(&export.classes, constructor.owner).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider class",
                        index: super::super::raw_index(constructor.owner),
                    },
                ),
            )?;

            (
                signatures
                    .binder_frame(&owner.type_params, 0)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Signature)?,
                parameter_ids(&owner.type_params),
            )
        }
        crate::LexicalDefinitionRoot::VariantConstructor(variant) => {
            let enumeration = super::super::arena_get(&export.enums, variant.enumeration()).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider enum",
                        index: super::super::raw_index(variant.enumeration()),
                    },
                ),
            )?;

            (
                signatures
                    .binder_frame(&enumeration.type_params, 0)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Signature)?,
                parameter_ids(&enumeration.type_params),
            )
        }
    };
    Ok(ProviderScope {
        root: entities
            .lexical_root(root)
            .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?,
        binders,
        flattened,
    })
}

fn flattened_function_parameters(genericity: &FunctionGenericity) -> Vec<TypeParamId> {
    match genericity {
        FunctionGenericity::Plain => Vec::new(),
        FunctionGenericity::Generic { parameters, .. } => parameter_ids(parameters),
        FunctionGenericity::OwnerParameterizedMethod {
            owner_parameters, ..
        } => parameter_ids(owner_parameters),
        FunctionGenericity::GenericMethod {
            owner_parameters,
            method_parameters,
            ..
        } => owner_parameters
            .iter()
            .chain(method_parameters.iter())
            .map(|parameter| parameter.id)
            .collect(),
    }
}

fn parameter_ids(parameters: &[TypeParamDecl]) -> Vec<TypeParamId> {
    parameters.iter().map(|parameter| parameter.id).collect()
}
