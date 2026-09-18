//! Projection of a default's provider scope and top-level envelope.

use super::super::signatures::HirInterfaceSignatureProjector;
use super::{
    body, entities::DefaultEntityProjector, errors::DefaultTemplateEnvelopeProjectionError,
    locals::TemplateLocalProjection, references,
};
use crate::{
    CanonicalBinderUseListV1, CanonicalCallableInterfacesV1, CanonicalTemplateValueParametersV1,
    ExportDefaultSourceId, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ExportHir,
    FunctionGenericity, HirSignatureBinder, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    TemplateReceiverV1, TemplateValueParameterV1, TypeParamDecl, TypeParamId,
};

pub(super) fn project(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_, '_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    position: u32,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, super::DefaultTemplateProductionError> {
    let key = ExportDefaultTemplateKeyV1::new(owner.declaration, position);
    project_inner(export, entities, callables, owner, key, source_id).map_err(|source| {
        super::DefaultTemplateProductionError::Template {
            key,
            source: Box::new(source),
        }
    })
}

fn project_inner(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_, '_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    key: ExportDefaultTemplateKeyV1,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
    let source = super::arena_get(&export.export_default_sources, source_id).ok_or(
        DefaultTemplateEnvelopeProjectionError::UnknownDefaultSource(super::default_source_id(
            source_id,
        )),
    )?;
    let template = super::arena_get(&export.export_default_exprs, source.expression).ok_or(
        DefaultTemplateEnvelopeProjectionError::UnknownDefaultExpression(super::raw_index(
            source.expression,
        )),
    )?;
    let provider = provider_scope(export, entities, template.definition_root)?;
    if template.type_parameters.len() != provider.flattened.len() {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
            expected: provider.flattened.len(),
            actual: template.type_parameters.len(),
        });
    }
    if let Some(position) = template
        .type_parameters
        .iter()
        .zip(&provider.flattened)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterIdentity { position });
    }
    if source.type_arguments.len() != provider.flattened.len() {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
            expected: provider.flattened.len(),
            actual: source.type_arguments.len(),
        });
    }
    let type_parameters = source
        .type_arguments
        .iter()
        .map(|&argument| {
            entities
                .type_key(argument, &owner.binders)
                .map_err(DefaultTemplateEnvelopeProjectionError::Provider)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let type_parameters = CanonicalBinderUseListV1::try_new(type_parameters)
        .map_err(DefaultTemplateEnvelopeProjectionError::BinderUse)?;
    let (locals, local_table) =
        TemplateLocalProjection::project(entities, template, &provider.binders)?;
    let body = body::project(
        entities,
        &locals,
        &provider.binders,
        template.origin,
        &template.statements,
        &template.value,
    )
    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?;
    let result = entities
        .type_key(template.result_type, &provider.binders)
        .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?;
    let receiver = template
        .receiver
        .map(|receiver| {
            TemplateReceiverV1::try_new(
                locals
                    .selector(receiver.local)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?,
                entities
                    .type_key(receiver.ty, &provider.binders)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?,
            )
            .map(OptionalTemplateReceiverV1::Present)
            .map_err(DefaultTemplateEnvelopeProjectionError::Receiver)
        })
        .transpose()?
        .unwrap_or(OptionalTemplateReceiverV1::Absent);
    let mut value_parameters = Vec::with_capacity(template.value_parameters.len());
    for (index, parameter) in template.value_parameters.iter().enumerate() {
        value_parameters.push(
            TemplateValueParameterV1::try_new(
                parameter.position,
                locals
                    .selector(parameter.local)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?,
            )
            .map_err(|source| {
                DefaultTemplateEnvelopeProjectionError::ValueParameter { index, source }
            })?,
        );
    }
    let value_parameters = CanonicalTemplateValueParametersV1::try_new(value_parameters)
        .map_err(DefaultTemplateEnvelopeProjectionError::ValueParameters)?;
    let owner_interface = callables.get(owner.declaration).ok_or(
        DefaultTemplateEnvelopeProjectionError::Provider(
            super::DefaultEntityProjectionError::MissingIdentity {
                kind: "default owner callable interface",
                index: super::owner_index(owner.local),
            },
        ),
    )?;
    let provider_owner = provider.root.declaration();
    let provider_interface =
        callables
            .get(provider_owner)
            .ok_or(DefaultTemplateEnvelopeProjectionError::Provider(
                super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "default provider callable interface",
                    index: super::owner_index(owner.local),
                },
            ))?;
    let reference_projection = references::ReferenceProjection {
        entities,
        source_owner: provider_owner,
        source_access: provider_interface.access(),
        target_owner: owner.declaration,
        target_access: owner_interface.access(),
        binders: &provider.binders,
    };
    let references = references::project(&reference_projection, &template.references)
        .map_err(DefaultTemplateEnvelopeProjectionError::References)?;
    let definition_origin =
        super::super::definition_sources::project_definition_source(export, template.origin)
            .map_err(DefaultTemplateEnvelopeProjectionError::DefinitionOrigin)?;
    ExportDefaultTemplateV1::try_new(
        key,
        provider.root,
        template.definition_path.clone(),
        local_table,
        body,
        result,
        template.allows_suspend.into(),
        type_parameters,
        receiver,
        value_parameters,
        references,
        definition_origin,
    )
    .map_err(DefaultTemplateEnvelopeProjectionError::Record)
}

struct ProviderScope {
    root: PersistentLexicalRootV1,
    binders: Vec<HirSignatureBinder>,
    flattened: Vec<TypeParamId>,
}

fn provider_scope(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_, '_>,
    root: crate::LexicalDefinitionRoot,
) -> Result<ProviderScope, DefaultTemplateEnvelopeProjectionError> {
    let signatures = HirInterfaceSignatureProjector::new(export);
    let (binders, flattened) = match root {
        crate::LexicalDefinitionRoot::Function(id) => {
            let function = super::arena_get(&export.functions, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider function",
                        index: super::raw_index(id),
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
            let constructor = super::arena_get(&export.struct_constructors, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider struct constructor",
                        index: super::raw_index(id),
                    },
                ),
            )?;
            let owner = super::arena_get(&export.structs, constructor.owner).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider struct",
                        index: super::raw_index(constructor.owner),
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
            let constructor = super::arena_get(&export.class_constructors, id).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider class constructor",
                        index: super::raw_index(id),
                    },
                ),
            )?;
            let owner = super::arena_get(&export.classes, constructor.owner).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider class",
                        index: super::raw_index(constructor.owner),
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
            let enumeration = super::arena_get(&export.enums, variant.enumeration()).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::Unknown {
                        kind: "default provider enum",
                        index: super::raw_index(variant.enumeration()),
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
