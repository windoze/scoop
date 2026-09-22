use super::*;
use scoop_wire::WireEncode;
use std::collections::{BTreeMap, BTreeSet};

struct ReferenceUses<'a, T> {
    source: &'a DefaultSourceReferenceV1<T>,
    uses: BTreeSet<ProtectedDefaultExpressionUseV1>,
}

struct References<'a, T> {
    records: BTreeMap<(&'a T, &'a ExportDefinitionSourceV1), ReferenceUses<'a, T>>,
}

impl<'a, T: Ord + Clone + WireEncode> References<'a, T> {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    fn observe(
        &mut self,
        source: &'a DefaultSourceReferenceV1<T>,
        context: DefaultReferenceContextV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root();
        let length = scoop_wire::encoded_length(source).map_err(invalid)?;
        meter
            .charge_work(
                length.saturating_mul(u64::from(self.records.len().max(1).ilog2()) + 1),
                &path,
            )
            .map_err(resource)?;
        meter.charge_collection_slots(1, &path).map_err(resource)?;
        let record = self
            .records
            .entry((source.target(), source.definition_origin()))
            .or_insert_with(|| ReferenceUses {
                source,
                uses: BTreeSet::new(),
            });
        if record.source.witness() != source.witness() {
            return Err(invalid(
                "repeated default reference has conflicting source access",
            ));
        }
        if let Some(usage) = context.expression_use() {
            work(meter, record.uses.len())?;
            meter.charge_collection_slots(1, &path).map_err(resource)?;
            record.uses.insert(usage);
        }
        Ok(())
    }

    fn finish(
        self,
        publication: &witness::Publication,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<ProtectedDefaultReferenceV1<T>>, Error> {
        resources::canonical(self.records.len(), meter)?;
        let mut records = Vec::new();
        for record in self.records.into_values() {
            let source = record.source;
            let length = scoop_wire::encoded_length(source).map_err(invalid)?;
            meter
                .charge_owned_bytes(length, &WirePath::root())
                .map_err(resource)?;
            meter
                .charge_work(length, &WirePath::root())
                .map_err(resource)?;
            resources::canonical(record.uses.len(), meter)?;
            let uses = CanonicalProtectedDefaultExpressionUsesV1::try_new(
                record.uses.into_iter().collect(),
            )
            .map_err(invalid)?;
            let witness = publication.reference(source.witness(), meter)?;
            resources::push(
                &mut records,
                ProtectedDefaultReferenceV1::new(
                    source.target().clone(),
                    source.definition_origin().clone(),
                    witness,
                    uses,
                ),
                meter,
            )?;
        }
        Ok(records)
    }
}

pub(super) fn project(
    source: &DefaultSourceTemplateV1,
    publication: &witness::Publication,
    meter: &mut BudgetMeter,
) -> Result<ProtectedDefaultReferenceSetV1, Error> {
    let closure = source
        .bind_reference_occurrences(meter, &WirePath::root())
        .map_err(invalid)?;
    let mut callables = References::new();
    let mut constructors = References::new();
    let mut types = References::new();
    let mut globals = References::new();
    let mut singletons = References::new();
    let mut fields = References::new();
    for occurrence in closure.occurrences() {
        let context = occurrence.context();
        use DefaultSourceReferenceRecordV1 as Record;
        match occurrence.source() {
            Record::Callable(source) => callables.observe(source, context, meter)?,
            Record::Constructor(source) => constructors.observe(source, context, meter)?,
            Record::Type(source) => types.observe(source, context, meter)?,
            Record::Global(source) => globals.observe(source, context, meter)?,
            Record::Singleton(source) => singletons.observe(source, context, meter)?,
            Record::Field(source) => fields.observe(source, context, meter)?,
        }
    }
    ProtectedDefaultReferenceSetV1::try_new(
        callables.finish(publication, meter)?,
        constructors.finish(publication, meter)?,
        types.finish(publication, meter)?,
        globals.finish(publication, meter)?,
        singletons.finish(publication, meter)?,
        fields.finish(publication, meter)?,
    )
    .map_err(invalid)
}
