use std::cmp::Ordering;

use super::*;
use crate::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    ExportDefaultAccessWitnessV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
    ProtectedDefaultAccessWitnessV1, ProtectedDefaultReferenceBodySemanticAuthority,
    ProtectedDefaultReferenceReceiverV1, ProtectedDefaultReferenceSetV1,
    ProtectedDefaultReferenceV1, compare_default_signature_reference_targets,
};
use scoop_wire::{WireEncode, WireError};

pub(super) fn inventory(
    candidate: &ProtectedDefaultReferenceSetV1,
    shared: &ExportDefaultReferenceSetV1,
    key: ProtectedDefaultTemplateKeyV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    entries(candidate.callables(), shared.callables(), key, meter)?;
    entries(candidate.constructors(), shared.constructors(), key, meter)?;
    entries(candidate.types(), shared.types(), key, meter)?;
    entries(candidate.globals(), shared.globals(), key, meter)?;
    entries(
        candidate.singleton_values(),
        shared.singleton_values(),
        key,
        meter,
    )?;
    entries(candidate.fields(), shared.fields(), key, meter)
}

fn entries<T: Eq + WireEncode>(
    candidate: &[ProtectedDefaultReferenceV1<T>],
    shared: &[ExportDefaultReferenceV1<T>],
    key: ProtectedDefaultTemplateKeyV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter.charge_work(
        candidate.len() as u64 + shared.len() as u64 + 1,
        &WirePath::root(),
    )?;
    if candidate.len() != shared.len() {
        return Err(Error::DefaultReferences(key));
    }
    for (candidate, shared) in candidate.iter().zip(shared) {
        contracts::charge_compare(candidate.target(), shared.target(), meter)?;
        contracts::charge_compare(
            candidate.definition_origin(),
            shared.definition_origin(),
            meter,
        )?;
        if candidate.target() != shared.target()
            || candidate.definition_origin() != shared.definition_origin()
        {
            return Err(Error::DefaultReferences(key));
        }
    }
    Ok(())
}

pub(super) struct Replay<'g, 'a, 's> {
    pub key: ProtectedDefaultTemplateKeyV1,
    pub shared: &'s ExportDefaultReferenceSetV1,
    pub witnesses: witnesses::Witnesses<'g, 'a>,
}

impl ProtectedDefaultReferenceBodySemanticAuthority<Error> for Replay<'_, '_, '_> {
    fn validate_default_reference_occurrence(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        witness: &ProtectedDefaultAccessWitnessV1,
        _receiver: ProtectedDefaultReferenceReceiverV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        if key != self.key {
            return Err(Error::DefaultContract(key));
        }
        // The shared reader has checked the identical body's receiver and access.
        // This lookup ties this occurrence to that exact declaration-side snapshot.
        macro_rules! find {
            ($records:expr, $compare:expr) => {
                find(
                    $records,
                    occurrence.definition_origin,
                    key,
                    $compare,
                    meter,
                    path,
                )?
            };
        }
        let expected = match occurrence.target {
            Target::Callable(target) => {
                find!(self.shared.callables(), |record, meter, path| target
                    .compare_to(record, meter, path))
            }
            Target::Constructor(target) => {
                find!(self.shared.constructors(), |record, meter, path| target
                    .compare_to(record, meter, path))
            }
            Target::Field(target) => find!(self.shared.fields(), |record, meter, path| target
                .compare_to(record, meter, path)),
            Target::Type(target) => find!(self.shared.types(), |record, meter, path| {
                compare_default_signature_reference_targets(target, record, meter, path)
            }),
            Target::Global(target) => find!(
                self.shared.globals(),
                |record, meter: &mut BudgetMeter, path| {
                    meter.charge_work(1, path)?;
                    Ok(target.cmp(record))
                }
            ),
            Target::Singleton(target) => find!(
                self.shared.singleton_values(),
                |record, meter: &mut BudgetMeter, path| {
                    meter.charge_work(1, path)?;
                    Ok(target.cmp(record))
                }
            ),
        };
        self.witnesses.validate(witness, expected, key, meter)
    }
}

fn find<'a, T>(
    records: &'a [ExportDefaultReferenceV1<T>],
    origin: &ExportDefinitionSourceV1,
    key: ProtectedDefaultTemplateKeyV1,
    mut compare: impl FnMut(&T, &mut BudgetMeter, &WirePath) -> Result<Ordering, WireError>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'a ExportDefaultAccessWitnessV1, Error> {
    let (mut low, mut high) = (0, records.len());
    while low < high {
        meter.charge_work(1, path)?;
        let index = low + (high - low) / 2;
        let record = &records[index];
        let mut order = compare(record.target(), meter, path)?;
        if order == Ordering::Equal {
            contracts::charge_compare(origin, record.definition_origin(), meter)?;
            order = origin.cmp(record.definition_origin());
        }
        match order {
            Ordering::Less => high = index,
            Ordering::Greater => low = index + 1,
            Ordering::Equal => return Ok(record.witness()),
        }
    }
    Err(Error::DefaultReferences(key))
}
