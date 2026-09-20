//! Projection of the definition-side access-reference closure.

use scoop_identity::CallableTemplateOrigin;

use super::entities::DefaultEntityProjector;
use crate::{
    DefaultConstructorRefV1, ExportDefaultAccessWitness, ExportDefaultAccessWitnessV1,
    ExportDefaultCallDomainV1, ExportDefaultCallableTarget, ExportDefaultCallableTargetV1,
    ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
    ExportDefaultReferences, ExportHir, HirSignatureBinder, PublicLookupAccessV1,
};

pub(super) struct ReferenceProjection<'a, 'hir, 'core, 'meter> {
    pub(super) entities: &'a DefaultEntityProjector<'hir, 'core, 'meter>,
    pub(super) source_owner: CallableTemplateOrigin,
    pub(super) source_access: PublicLookupAccessV1,
    pub(super) target_owner: CallableTemplateOrigin,
    pub(super) target_access: PublicLookupAccessV1,
    pub(super) binders: &'a [HirSignatureBinder],
}

pub(super) fn project(
    projection: &ReferenceProjection<'_, '_, '_, '_>,
    references: &ExportDefaultReferences,
) -> Result<ExportDefaultReferenceSetV1, super::DefaultReferenceProjectionError> {
    let mut callables = Vec::with_capacity(references.callables.len());
    for (index, reference) in references.callables.iter().enumerate() {
        let target = callable_target(projection, reference.target)?;
        callables.push(ExportDefaultReferenceV1::new(
            target,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Callable,
                index,
            )?,
        ));
    }

    let mut constructors = Vec::with_capacity(references.constructors.len());
    for (index, reference) in references.constructors.iter().enumerate() {
        let target = match reference.target {
            crate::ExportDefaultConstructorTarget::Struct(application) => projection
                .entities
                .struct_constructor(application, projection.binders)?,
            crate::ExportDefaultConstructorTarget::Class(application) => projection
                .entities
                .class_constructor(application, projection.binders)?,
            crate::ExportDefaultConstructorTarget::Variant(variant) => {
                let variant = projection.entities.variant(variant, projection.binders)?;
                DefaultConstructorRefV1::Variant {
                    declaration: variant.declaration(),
                    owner_type: variant.owner_type().clone(),
                }
            }
        };
        constructors.push(ExportDefaultReferenceV1::new(
            target,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Constructor,
                index,
            )?,
        ));
    }

    let mut types = Vec::with_capacity(references.types.len());
    for (index, reference) in references.types.iter().enumerate() {
        types.push(ExportDefaultReferenceV1::new(
            projection
                .entities
                .type_key(reference.target, projection.binders)?,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Type,
                index,
            )?,
        ));
    }

    let mut globals = Vec::with_capacity(references.globals.len());
    for (index, reference) in references.globals.iter().enumerate() {
        globals.push(ExportDefaultReferenceV1::new(
            projection.entities.global_property(reference.target)?,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Global,
                index,
            )?,
        ));
    }

    let mut singleton_values = Vec::with_capacity(references.singleton_values.len());
    for (index, reference) in references.singleton_values.iter().enumerate() {
        singleton_values.push(ExportDefaultReferenceV1::new(
            projection.entities.singleton_id(reference.target)?,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Singleton,
                index,
            )?,
        ));
    }

    let mut fields = Vec::with_capacity(references.fields.len());
    for (index, reference) in references.fields.iter().enumerate() {
        fields.push(ExportDefaultReferenceV1::new(
            projection
                .entities
                .field(reference.target, projection.binders)?,
            origin(projection.entities.export(), reference.origin)?,
            witness(
                projection,
                &reference.witness,
                ExportDefaultReferenceKindV1::Field,
                index,
            )?,
        ));
    }

    canonicalize(&mut callables);
    canonicalize(&mut constructors);
    canonicalize(&mut types);
    canonicalize(&mut globals);
    canonicalize(&mut singleton_values);
    canonicalize(&mut fields);
    ExportDefaultReferenceSetV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singleton_values,
        fields,
    )
    .map_err(super::DefaultReferenceProjectionError::Set)
}

fn canonicalize<T: Ord>(records: &mut Vec<T>) {
    records.sort_unstable();
    records.dedup();
}

fn callable_target(
    projection: &ReferenceProjection<'_, '_, '_, '_>,
    target: ExportDefaultCallableTarget,
) -> Result<ExportDefaultCallableTargetV1, super::DefaultReferenceProjectionError> {
    let entities = projection.entities;
    let export = entities.export();
    let binders = projection.binders;
    Ok(match target {
        ExportDefaultCallableTarget::Callable(callable) => {
            ExportDefaultCallableTargetV1::Callable(entities.callable(callable, binders)?)
        }
        ExportDefaultCallableTarget::ImportedCore(callable) => {
            ExportDefaultCallableTargetV1::Callable(entities.imported_callable(callable)?)
        }
        ExportDefaultCallableTarget::ImportedDependency(callable) => {
            ExportDefaultCallableTargetV1::Callable(
                entities.imported_dependency_callable(callable)?,
            )
        }
        ExportDefaultCallableTarget::Bound(bound) => {
            ExportDefaultCallableTargetV1::Bound(entities.bound_callable(bound, binders)?)
        }
        ExportDefaultCallableTarget::DerivedEquality(id) => {
            let application = super::arena_get(&export.derived_equality_applications, id).ok_or(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "derived equality application",
                    index: super::raw_index(id),
                },
            )?;
            ExportDefaultCallableTargetV1::DerivedEquality {
                owner_type: entities.type_key(application.owner_ty, binders)?,
            }
        }
        ExportDefaultCallableTarget::LocalFunction(id) => {
            let function = super::arena_get(&export.local_functions, id).ok_or(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "local function",
                    index: super::raw_index(id),
                },
            )?;
            ExportDefaultCallableTargetV1::LocalFunction {
                declaration: entities.source_callable_declaration(function.function)?,
            }
        }
        ExportDefaultCallableTarget::Lambda(id) => {
            let lambda = super::arena_get(&export.lambdas, id).ok_or(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "lambda",
                    index: super::raw_index(id),
                },
            )?;
            ExportDefaultCallableTargetV1::Lambda {
                body: entities.generated_function_id(lambda.function)?,
            }
        }
        ExportDefaultCallableTarget::AnonymousFunction(id) => {
            let function = super::arena_get(&export.anonymous_functions, id).ok_or(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "anonymous function",
                    index: super::raw_index(id),
                },
            )?;
            ExportDefaultCallableTargetV1::AnonymousFunction {
                body: entities.generated_function_id(function.function)?,
            }
        }
        ExportDefaultCallableTarget::CallableReference(id) => {
            let reference = super::arena_get(&export.callable_references, id).ok_or(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "callable reference",
                    index: super::raw_index(id),
                },
            )?;
            ExportDefaultCallableTargetV1::CallableReference {
                invoke: entities.callable_reference_invoke(
                    reference.definition_root,
                    &reference.definition_path,
                )?,
            }
        }
        ExportDefaultCallableTarget::FunctionAddress(function) => {
            ExportDefaultCallableTargetV1::FunctionAddress {
                declaration: entities.callable_declaration(function)?,
            }
        }
    })
}

fn witness(
    projection: &ReferenceProjection<'_, '_, '_, '_>,
    witness: &ExportDefaultAccessWitness,
    kind: ExportDefaultReferenceKindV1,
    index: usize,
) -> Result<ExportDefaultAccessWitnessV1, super::DefaultReferenceProjectionError> {
    let actual = projection.entities.parameter_owner(witness.owner)?;
    if actual != projection.source_owner {
        return Err(super::DefaultReferenceProjectionError::Owner {
            kind,
            index,
            expected: projection.source_owner,
            actual,
        });
    }
    if !witness.target_domain.is_universal() || !witness.call_domain.direct.0.is_universal() {
        return Err(super::DefaultReferenceProjectionError::RestrictedTarget { kind, index });
    }
    let source_domain_matches = match (projection.source_access, witness.call_domain.slot.as_ref())
    {
        (PublicLookupAccessV1::DirectOnly, None) => true,
        (PublicLookupAccessV1::PublicSlot, Some(slot)) => slot.0.is_universal(),
        _ => false,
    };
    if !source_domain_matches {
        return Err(super::DefaultReferenceProjectionError::InvalidCallDomain { kind, index });
    }
    let call_domain = match projection.target_access {
        PublicLookupAccessV1::DirectOnly => ExportDefaultCallDomainV1::DirectPublic,
        PublicLookupAccessV1::PublicSlot => ExportDefaultCallDomainV1::DirectAndPublicSlot,
    };
    Ok(ExportDefaultAccessWitnessV1::new(
        projection.target_owner,
        call_domain,
    ))
}

fn origin(
    export: &ExportHir,
    origin: crate::DefinitionOrigin,
) -> Result<crate::ExportDefinitionSourceV1, super::DefaultReferenceProjectionError> {
    super::super::definition_sources::project_definition_source(export, origin)
        .map_err(super::DefaultReferenceProjectionError::DefinitionOrigin)
}
