use super::super::binding::charge_path;
use super::*;
mod errors;
pub use errors::DefaultSourceValueDomainBindingError;
type BindingError = DefaultSourceValueDomainBindingError;

/// Domain equality for every constructor, global, singleton and field occurrence.
/// Other reference kinds, operation typing, receivers and coverage remain separate.
#[derive(Debug)]
pub struct BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.declarations
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_value_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let foundation = declarations
            .origins()
            .parameters()
            .members()
            .nominals
            .foundation;
        if !std::ptr::eq(self.current.foundation, foundation) {
            return Err(BindingError::Foundation {
                expected: self.current.provider(),
                actual: declarations.provider(),
            });
        }
        meter.check_table_entries(declarations.declarations().len() as u64, &path)?;
        for (index, declaration) in declarations.declarations().iter().enumerate() {
            charge_path(meter, &path, 1)?;
            let path = path.clone().index(index as u64);
            meter.charge_nodes(1, &path)?;
            meter.charge_edges(1, &path)?;
            meter.charge_work(1, &path)?;
            let occurrences = declaration.references().occurrences();
            meter.check_table_entries(occurrences.len() as u64, &path)?;
            for occurrence in occurrences {
                meter.charge_work(1, &path)?;
                let record = occurrence.source();
                let target = match record {
                    DefaultSourceReferenceRecordV1::Constructor(r) => {
                        Target::Constructor(r.target())
                    }
                    DefaultSourceReferenceRecordV1::Global(r) => Target::Global(*r.target()),
                    DefaultSourceReferenceRecordV1::Singleton(r) => Target::Singleton(*r.target()),
                    DefaultSourceReferenceRecordV1::Field(r) => Target::Field(r.target()),
                    DefaultSourceReferenceRecordV1::Callable(_)
                    | DefaultSourceReferenceRecordV1::Type(_) => continue,
                };
                let kind = record.kind();
                let field = match kind {
                    ExportDefaultReferenceKindV1::Constructor => 2,
                    ExportDefaultReferenceKindV1::Global => 4,
                    ExportDefaultReferenceKindV1::Singleton => 5,
                    ExportDefaultReferenceKindV1::Field => 6,
                    ExportDefaultReferenceKindV1::Callable => 1,
                    ExportDefaultReferenceKindV1::Type => 3,
                };
                charge_path(meter, &path, 4)?;
                let path = path
                    .clone()
                    .field(11)
                    .field(field)
                    .index(u64::from(occurrence.index()))
                    .field(1);
                let expected =
                    self.value_source_domain_at(target, meter, &path)
                        .map_err(|error| {
                            BindingError::target(declaration.key(), kind, occurrence.index(), error)
                        })?;
                let actual = record.witness().target_domain();
                let cost = scoop_wire::encoded_length(&expected)
                    .and_then(|expected| {
                        scoop_wire::encoded_length(actual)
                            .map(|actual| expected.saturating_add(actual))
                    })
                    .map_err(BindingError::Encoding)?;
                meter.charge_work(cost, &path)?;
                if actual != &expected {
                    return Err(BindingError::Witness {
                        key: declaration.key(),
                        kind,
                        index: occurrence.index(),
                    });
                }
            }
        }
        Ok(BoundNominalDefaultValueDomainsV1 { declarations })
    }
}
