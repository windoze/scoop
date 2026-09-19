use std::cmp::Ordering;

use scoop_wire::{BudgetMeter, WirePath};

use super::ordering::{ReferenceTarget, compare_keys};
use super::*;
use crate::ExportDefaultReferenceTargetResolutionError as TargetError;

impl DecodedProtectedDefaultReferenceSetV1 {
    pub fn resolve<R: ProtectedDefaultReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedDefaultReferenceSetV1, ProtectedDefaultReferenceSetResolutionError<E>>
    {
        use ProtectedDefaultReferenceKindV1 as Kind;
        use ProtectedDefaultReferenceResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(ProtectedDefaultReferenceSetResolutionError::Resource)?;
        meter
            .charge_work(1, &WirePath::root())
            .map_err(ProtectedDefaultReferenceSetResolutionError::Resource)?;
        let callables = resolve_set(
            self.callables,
            resolver,
            meter,
            Kind::Callable,
            |target, resolver, meter| {
                target.charge_resolution(meter).map_err(Error::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| Error::Target(TargetError::Callable(e)))
            },
        )?;
        let constructors = resolve_set(
            self.constructors,
            resolver,
            meter,
            Kind::Constructor,
            |target, resolver, meter| {
                target.charge_resolution(meter).map_err(Error::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| Error::Target(TargetError::Constructor(e)))
            },
        )?;
        let types = resolve_set(
            self.types,
            resolver,
            meter,
            Kind::Type,
            |target, resolver, meter| {
                target.charge_resolution(meter).map_err(Error::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| Error::Target(TargetError::Type(e)))
            },
        )?;
        let globals = resolve_set(
            self.globals,
            resolver,
            meter,
            Kind::Global,
            |target, resolver, _| {
                resolver
                    .resolve(target)
                    .map_err(|e| Error::Target(TargetError::Global(e)))
            },
        )?;
        let singleton_values = resolve_set(
            self.singleton_values,
            resolver,
            meter,
            Kind::Singleton,
            |target, resolver, _| {
                resolver
                    .resolve(target)
                    .map_err(|e| Error::Target(TargetError::Singleton(e)))
            },
        )?;
        let fields = resolve_set(
            self.fields,
            resolver,
            meter,
            Kind::Field,
            |target, resolver, meter| {
                target.charge_resolution(meter).map_err(Error::Resource)?;
                target
                    .resolve(resolver)
                    .map_err(|e| Error::Target(TargetError::Field(e)))
            },
        )?;
        Ok(ProtectedDefaultReferenceSetV1 {
            callables,
            constructors,
            types,
            globals,
            singleton_values,
            fields,
        })
    }
}

fn resolve_set<R, E, D, T>(
    records: Vec<DecodedProtectedDefaultReferenceV1<D>>,
    resolver: &mut R,
    meter: &mut BudgetMeter,
    kind: ProtectedDefaultReferenceKindV1,
    mut resolve_target: impl FnMut(
        D,
        &mut R,
        &mut BudgetMeter,
    ) -> Result<T, ProtectedDefaultReferenceResolutionError<E>>,
) -> Result<Vec<ProtectedDefaultReferenceV1<T>>, ProtectedDefaultReferenceSetResolutionError<E>>
where
    R: ProtectedDefaultReferenceResolver<E>,
    T: ReferenceTarget,
{
    use ProtectedDefaultReferenceSetBuildError as BuildError;
    use ProtectedDefaultReferenceSetResolutionError as Error;
    u32::try_from(records.len()).map_err(|_| Error::Build(BuildError::TooMany(kind)))?;
    let mut resolved = Vec::new();
    let path = WirePath::root();
    meter
        .charge_collection_slots(records.len() as u64, &path)
        .map_err(Error::Resource)?;
    meter
        .try_reserve_collection_slots(&mut resolved, records.len(), &path)
        .map_err(Error::Resource)?;
    for (index, record) in records.into_iter().enumerate() {
        let record = record
            .resolve_with(resolver, meter, &mut resolve_target)
            .map_err(|error| Error::Record { kind, index, error })?;
        if let Some(previous) = resolved.last() {
            match compare_keys(previous, &record, meter)? {
                Ordering::Equal => return Err(Error::Build(BuildError::Duplicate { kind, index })),
                Ordering::Greater => {
                    return Err(Error::Build(BuildError::NonCanonicalOrder { kind, index }));
                }
                Ordering::Less => {}
            }
        }
        resolved.push(record);
    }
    Ok(resolved)
}
