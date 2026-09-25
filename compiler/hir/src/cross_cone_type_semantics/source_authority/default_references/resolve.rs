use super::*;
use crate::*;
use DefaultSourceReferenceResolutionError as RecordError;
use DefaultSourceReferencesResolutionError as Error;
use ExportDefaultReferenceTargetResolutionError as TargetError;
use scoop_wire::WirePath;

pub trait DefaultSourceReferenceResolver<E>:
    DefaultExpressionReferenceResolver<E> + DefaultSourceAccessWitnessResolver<E>
{
}
impl<R, E> DefaultSourceReferenceResolver<E> for R where
    R: DefaultExpressionReferenceResolver<E> + DefaultSourceAccessWitnessResolver<E>
{
}

impl DecodedDefaultSourceReferencesV1 {
    pub fn resolve<R: DefaultSourceReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultSourceReferencesV1, Error<E>> {
        let callables = resolve_sequence(
            self.callables,
            resolver,
            ExportDefaultReferenceKindV1::Callable,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Callable(e)))
            },
        )?;
        let constructors = resolve_sequence(
            self.constructors,
            resolver,
            ExportDefaultReferenceKindV1::Constructor,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Constructor(e)))
            },
        )?;
        let types = resolve_sequence(
            self.types,
            resolver,
            ExportDefaultReferenceKindV1::Type,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Type(e)))
            },
        )?;
        let globals = resolve_sequence(
            self.globals,
            resolver,
            ExportDefaultReferenceKindV1::Global,
            |target, resolver| {
                resolver
                    .resolve(target)
                    .map_err(|e| RecordError::Target(TargetError::Global(e)))
            },
        )?;
        let singleton_values = resolve_sequence(
            self.singleton_values,
            resolver,
            ExportDefaultReferenceKindV1::Singleton,
            |target, resolver| {
                resolver
                    .resolve(target)
                    .map_err(|e| RecordError::Target(TargetError::Singleton(e)))
            },
        )?;
        let fields = resolve_sequence(
            self.fields,
            resolver,
            ExportDefaultReferenceKindV1::Field,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Field(e)))
            },
        )?;
        Ok(DefaultSourceReferencesV1 {
            callables,
            constructors,
            types,
            globals,
            singleton_values,
            fields,
        })
    }
}

fn resolve_sequence<R: DefaultSourceReferenceResolver<E>, E, D, T>(
    records: Vec<DecodedDefaultSourceReferenceV1<D>>,
    resolver: &mut R,

    kind: ExportDefaultReferenceKindV1,
    mut target: impl FnMut(D, &mut R) -> Result<T, RecordError<E>>,
) -> Result<Vec<DefaultSourceReferenceV1<T>>, Error<E>> {
    u32::try_from(records.len())
        .map_err(|_| Error::Build(DefaultSourceReferencesBuildError::TooMany(kind)))?;
    let path = WirePath::root();

    let mut result = Vec::new();

    scoop_wire::allocation::try_reserve(&mut result, records.len(), &path)
        .map_err(Error::Resource)?;
    for (index, record) in records.into_iter().enumerate() {
        let resolved = (|| {
            let target = target(record.target, resolver)?;

            let origin = record
                .definition_origin
                .resolve(resolver)
                .map_err(RecordError::DefinitionOrigin)?;
            let witness = record
                .witness
                .resolve(resolver)
                .map_err(RecordError::Witness)?;
            Ok(DefaultSourceReferenceV1::new(target, origin, witness))
        })()
        .map_err(|error| Error::Record { kind, index, error })?;
        result.push(resolved);
    }
    Ok(result)
}
