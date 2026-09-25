use std::cmp::Ordering;

use super::*;
use crate::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    ExportDefaultAccessWitnessV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
    ProtectedDefaultAccessWitnessV1, ProtectedDefaultReferenceBodySemanticAuthority,
    ProtectedDefaultReferenceReceiverV1, ProtectedDefaultReferenceSetV1,
    ProtectedDefaultReferenceV1, compare_default_signature_reference_targets,
};
use scoop_wire::WireError;

pub(super) fn inventory(
    candidate: &ProtectedDefaultReferenceSetV1,
    shared: &ExportDefaultReferenceSetV1,
    key: ProtectedDefaultTemplateKeyV1,
) -> Result<(), Error> {
    entries(candidate.callables(), shared.callables(), key)?;
    entries(candidate.constructors(), shared.constructors(), key)?;
    entries(candidate.types(), shared.types(), key)?;
    entries(candidate.globals(), shared.globals(), key)?;
    entries(candidate.singleton_values(), shared.singleton_values(), key)?;
    entries(candidate.fields(), shared.fields(), key)
}

fn entries<T: Eq>(
    candidate: &[ProtectedDefaultReferenceV1<T>],
    shared: &[ExportDefaultReferenceV1<T>],
    key: ProtectedDefaultTemplateKeyV1,
) -> Result<(), Error> {
    if candidate.len() != shared.len() {
        return Err(Error::DefaultReferences(key));
    }
    for (candidate, shared) in candidate.iter().zip(shared) {
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

        path: &WirePath,
    ) -> Result<(), Error> {
        if key != self.key {
            return Err(Error::DefaultContract(key));
        }
        // The shared reader has checked the identical body's receiver and access.
        // This lookup ties this occurrence to that exact declaration-side snapshot.
        macro_rules! find {
            ($records:expr, $compare:expr) => {
                find($records, occurrence.definition_origin, key, $compare, path)?
            };
        }
        let expected = match occurrence.target {
            Target::Callable(target) => {
                find!(self.shared.callables(), |record, path| target
                    .compare_to(record, path))
            }
            Target::Constructor(target) => {
                find!(self.shared.constructors(), |record, path| target
                    .compare_to(record, path))
            }
            Target::Field(target) => find!(self.shared.fields(), |record, path| target
                .compare_to(record, path)),
            Target::Type(target) => find!(self.shared.types(), |record, path| {
                compare_default_signature_reference_targets(target, record, path)
            }),
            Target::Global(target) => find!(self.shared.globals(), |record, _path| Ok(
                target.cmp(record)
            )),
            Target::Singleton(target) => find!(self.shared.singleton_values(), |record, _path| Ok(
                target.cmp(record)
            )),
        };
        self.witnesses.validate(witness, expected, key)
    }
}

fn find<'a, T>(
    records: &'a [ExportDefaultReferenceV1<T>],
    origin: &ExportDefinitionSourceV1,
    key: ProtectedDefaultTemplateKeyV1,
    mut compare: impl FnMut(&T, &WirePath) -> Result<Ordering, WireError>,

    path: &WirePath,
) -> Result<&'a ExportDefaultAccessWitnessV1, Error> {
    let (mut low, mut high) = (0, records.len());
    while low < high {
        let index = low + (high - low) / 2;
        let record = &records[index];
        let mut order = compare(record.target(), path)?;
        if order == Ordering::Equal {
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
