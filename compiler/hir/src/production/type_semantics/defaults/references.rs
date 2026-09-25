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
    ) -> Result<(), Error> {
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
            record.uses.insert(usage);
        }
        Ok(())
    }

    fn finish(
        self,
        publication: &witness::Publication,
    ) -> Result<Vec<ProtectedDefaultReferenceV1<T>>, Error> {
        let mut records = Vec::new();
        for record in self.records.into_values() {
            let source = record.source;

            let uses = CanonicalProtectedDefaultExpressionUsesV1::try_new(
                record.uses.into_iter().collect(),
            )
            .map_err(invalid)?;
            let witness = publication.reference(source.witness())?;
            resources::push(
                &mut records,
                ProtectedDefaultReferenceV1::new(
                    source.target().clone(),
                    source.definition_origin().clone(),
                    witness,
                    uses,
                ),
            )?;
        }
        Ok(records)
    }
}

pub(super) fn project(
    source: &DefaultSourceTemplateV1,
    publication: &witness::Publication,
) -> Result<ProtectedDefaultReferenceSetV1, Error> {
    let closure = source
        .bind_reference_occurrences(&WirePath::root())
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
            Record::Callable(source) => callables.observe(source, context)?,
            Record::Constructor(source) => constructors.observe(source, context)?,
            Record::Type(source) => types.observe(source, context)?,
            Record::Global(source) => globals.observe(source, context)?,
            Record::Singleton(source) => singletons.observe(source, context)?,
            Record::Field(source) => fields.observe(source, context)?,
        }
    }
    ProtectedDefaultReferenceSetV1::try_new(
        callables.finish(publication)?,
        constructors.finish(publication)?,
        types.finish(publication)?,
        globals.finish(publication)?,
        singletons.finish(publication)?,
        fields.finish(publication)?,
    )
    .map_err(invalid)
}
