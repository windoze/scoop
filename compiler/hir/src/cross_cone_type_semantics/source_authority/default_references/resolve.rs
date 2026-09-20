use super::*;
use crate::*;
use DefaultSourceReferenceResolutionError as RecordError;
use DefaultSourceReferencesResolutionError as Error;
use ExportDefaultReferenceTargetResolutionError as TargetError;
use scoop_wire::{BudgetMeter, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceReferencesV1, Error<E>> {
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let callables = resolve_sequence(
            self.callables,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Callable,
            |target, resolver, meter| {
                target
                    .charge_resolution(meter)
                    .map_err(RecordError::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Callable(e)))
            },
        )?;
        let constructors = resolve_sequence(
            self.constructors,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Constructor,
            |target, resolver, meter| {
                target
                    .charge_resolution(meter)
                    .map_err(RecordError::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Constructor(e)))
            },
        )?;
        let types = resolve_sequence(
            self.types,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Type,
            |target, resolver, meter| {
                target
                    .charge_resolution(meter)
                    .map_err(RecordError::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| RecordError::Target(TargetError::Type(e)))
            },
        )?;
        let globals = resolve_sequence(
            self.globals,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Global,
            |target, resolver, meter| {
                meter
                    .charge_work(1, &WirePath::root())
                    .map_err(RecordError::Resource)?;
                resolver
                    .resolve(target)
                    .map_err(|e| RecordError::Target(TargetError::Global(e)))
            },
        )?;
        let singleton_values = resolve_sequence(
            self.singleton_values,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Singleton,
            |target, resolver, meter| {
                meter
                    .charge_work(1, &WirePath::root())
                    .map_err(RecordError::Resource)?;
                resolver
                    .resolve(target)
                    .map_err(|e| RecordError::Target(TargetError::Singleton(e)))
            },
        )?;
        let fields = resolve_sequence(
            self.fields,
            resolver,
            meter,
            ExportDefaultReferenceKindV1::Field,
            |target, resolver, meter| {
                target
                    .charge_resolution(meter)
                    .map_err(RecordError::Resource)?;
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
    meter: &mut BudgetMeter,
    kind: ExportDefaultReferenceKindV1,
    mut target: impl FnMut(D, &mut R, &mut BudgetMeter) -> Result<T, RecordError<E>>,
) -> Result<Vec<DefaultSourceReferenceV1<T>>, Error<E>> {
    u32::try_from(records.len())
        .map_err(|_| Error::Build(DefaultSourceReferencesBuildError::TooMany(kind)))?;
    let path = WirePath::root();
    let mut result = Vec::new();
    meter
        .charge_collection_slots(records.len() as u64, &path)
        .map_err(Error::Resource)?;
    meter
        .try_reserve_collection_slots(&mut result, records.len(), &path)
        .map_err(Error::Resource)?;
    for (index, record) in records.into_iter().enumerate() {
        let at = path.clone().index(index as u64);
        let resolved = (|| {
            meter.charge_nodes(1, &at).map_err(RecordError::Resource)?;
            meter.charge_work(1, &at).map_err(RecordError::Resource)?;
            let target = target(record.target, resolver, meter)?;
            record
                .definition_origin
                .charge_resolution_at(meter, &at, 3)
                .map_err(RecordError::Resource)?;
            let origin = record
                .definition_origin
                .resolve(resolver)
                .map_err(RecordError::DefinitionOrigin)?;
            let witness = record
                .witness
                .resolve(resolver, meter)
                .map_err(RecordError::Witness)?;
            Ok(DefaultSourceReferenceV1::new(target, origin, witness))
        })()
        .map_err(|error| Error::Record { kind, index, error })?;
        result.push(resolved);
    }
    Ok(result)
}
