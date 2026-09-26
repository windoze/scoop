//! Projection of the typed direct-reference closure.

use super::entities::DefaultEntityProjector;
use crate::{
    DefaultConstructorRefV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
    ExportDefaultReferences, ExportHir, HirSignatureBinder,
};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    binders: &[HirSignatureBinder],
    references: &ExportDefaultReferences,
) -> Result<ExportDefaultReferenceSetV1, super::DefaultReferenceProjectionError> {
    let mut callables = Vec::with_capacity(references.callables.len());
    for reference in &references.callables {
        let target = entities.reference_callable(reference.target, binders)?;
        callables.push(ExportDefaultReferenceV1::new(
            target,
            origin(entities.export(), reference.origin)?,
        ));
    }

    let mut constructors = Vec::with_capacity(references.constructors.len());
    for reference in &references.constructors {
        let target = match reference.target {
            crate::ExportDefaultConstructorTarget::ImportedVariant {
                variant,
                owner_type,
            } => DefaultConstructorRefV1::Variant {
                declaration: variant,
                owner_type: entities.type_key(owner_type, binders)?,
            },
            crate::ExportDefaultConstructorTarget::Imported {
                declaration,
                owner_type,
            } => entities.imported_constructor(declaration, owner_type, binders)?,
            crate::ExportDefaultConstructorTarget::Struct(application) => {
                entities.struct_constructor(application, binders)?
            }
            crate::ExportDefaultConstructorTarget::Class(application) => {
                entities.class_constructor(application, binders)?
            }
            crate::ExportDefaultConstructorTarget::Variant(variant) => {
                let variant = entities.variant(variant, binders)?;
                DefaultConstructorRefV1::Variant {
                    declaration: variant.declaration(),
                    owner_type: variant.owner_type().clone(),
                }
            }
        };
        constructors.push(ExportDefaultReferenceV1::new(
            target,
            origin(entities.export(), reference.origin)?,
        ));
    }

    let mut types = Vec::with_capacity(references.types.len());
    for reference in &references.types {
        types.push(ExportDefaultReferenceV1::new(
            entities.reference_type(reference.target, binders)?,
            origin(entities.export(), reference.origin)?,
        ));
    }

    let mut globals = Vec::with_capacity(references.globals.len());
    for reference in &references.globals {
        globals.push(ExportDefaultReferenceV1::new(
            entities.global_property(reference.target)?,
            origin(entities.export(), reference.origin)?,
        ));
    }

    let mut singleton_values = Vec::with_capacity(references.singleton_values.len());
    for reference in &references.singleton_values {
        singleton_values.push(ExportDefaultReferenceV1::new(
            entities.singleton_id(reference.target)?,
            origin(entities.export(), reference.origin)?,
        ));
    }

    let mut fields = Vec::with_capacity(references.fields.len());
    for reference in &references.fields {
        fields.push(ExportDefaultReferenceV1::new(
            entities.field(reference.target, binders)?,
            origin(entities.export(), reference.origin)?,
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

fn origin(
    export: &ExportHir,
    origin: crate::DefinitionOrigin,
) -> Result<crate::ExportDefinitionSourceV1, super::DefaultReferenceProjectionError> {
    super::super::definition_sources::project_definition_source(export, origin)
        .map_err(super::DefaultReferenceProjectionError::DefinitionOrigin)
}
