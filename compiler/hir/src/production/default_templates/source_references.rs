//! Project occurrences alongside their source body, using the same sealed graph.
use super::entities::DefaultEntityProjector;
use crate::*;
use scoop_identity::CallableTemplateOrigin;
mod errors;
use DefaultSourceReferencesProductionError as Error;
pub use errors::DefaultSourceReferencesProductionError;

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    provider: CallableTemplateOrigin,
    binders: &[HirSignatureBinder],
    source: &ExportDefaultReferences,
) -> Result<DefaultSourceReferencesV1, Error> {
    let callables = sequence(
        entities,
        provider,
        source
            .callables
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Callable,
        |target| entities.reference_callable(target, binders),
    )?;
    let constructors = sequence(
        entities,
        provider,
        source
            .constructors
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Constructor,
        |target| match target {
            ExportDefaultConstructorTarget::Struct(application) => {
                entities.struct_constructor(application, binders)
            }
            ExportDefaultConstructorTarget::Class(application) => {
                entities.class_constructor(application, binders)
            }
            ExportDefaultConstructorTarget::Variant(variant) => {
                let application =
                    super::arena_get(&entities.export().enum_applications, variant.application())
                        .ok_or(DefaultEntityProjectionError::Unknown {
                        kind: "enum application",
                        index: super::raw_index(variant.application()),
                    })?;
                Ok(DefaultConstructorRefV1::Variant {
                    declaration: entities.variant_id(variant.declaration())?,
                    owner_type: entities.type_key(application.canonical_type, binders)?,
                })
            }
        },
    )?;
    let types = sequence(
        entities,
        provider,
        source
            .types
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Type,
        |target| entities.reference_type(target, binders),
    )?;
    let globals = sequence(
        entities,
        provider,
        source
            .globals
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Global,
        |target| entities.global_property(target),
    )?;
    let singleton_values = sequence(
        entities,
        provider,
        source
            .singleton_values
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Singleton,
        |target| entities.singleton_id(target),
    )?;
    let fields = sequence(
        entities,
        provider,
        source
            .fields
            .iter()
            .map(|r| (r.target, r.origin, &r.witness)),
        ExportDefaultReferenceKindV1::Field,
        |target| entities.field(target, binders),
    )?;
    DefaultSourceReferencesV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singleton_values,
        fields,
    )
    .map_err(Error::Build)
}

fn sequence<'a, S, T>(
    entities: &DefaultEntityProjector<'_>,
    provider: CallableTemplateOrigin,
    source: impl ExactSizeIterator<Item = (S, DefinitionOrigin, &'a ExportDefaultAccessWitness)>,
    kind: ExportDefaultReferenceKindV1,
    mut target: impl FnMut(S) -> Result<T, DefaultEntityProjectionError>,
) -> Result<Vec<DefaultSourceReferenceV1<T>>, Error> {
    u32::try_from(source.len())
        .map_err(|_| Error::Build(DefaultSourceReferencesBuildError::TooMany(kind)))?;

    let mut records = Vec::with_capacity(source.len());
    for (index, (source_target, source_origin, source_witness)) in source.enumerate() {
        let actual = entities.parameter_owner(source_witness.owner)?;
        if actual != provider {
            return Err(Error::Provider {
                kind,
                index,
                expected: provider,
                actual,
            });
        }
        let target = target(source_target)?;

        let origin = super::super::definition_sources::project_definition_source(
            entities.export(),
            source_origin,
        )
        .map_err(Error::DefinitionOrigin)?;
        let witness =
            { DefaultSourceAccessWitnessV1::from_export_hir(entities.export(), source_witness) }
                .map_err(Error::Access)?;
        records.push(DefaultSourceReferenceV1::new(target, origin, witness));
    }
    Ok(records)
}
