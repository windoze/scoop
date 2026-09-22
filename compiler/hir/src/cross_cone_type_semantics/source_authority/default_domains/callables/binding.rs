use super::super::binding::charge_path;
use super::*;
mod errors;
pub use errors::DefaultSourceCallableDomainBindingError;
type BindingError = DefaultSourceCallableDomainBindingError;

/// Complete callable target-domain equality. Receiver, operation, nested ABI,
/// profile and call-domain coverage are separate obligations.
#[derive(Debug)]
pub struct BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.declarations
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_callable_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
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
            let shape = declaration.provider_binders();
            let frame_count = usize::from(shape.nominal_owner_binder_arity() != 0)
                + usize::from(shape.callable_own_binder_arity() != 0);
            meter.charge_heap(
                (frame_count * std::mem::size_of::<std::num::NonZeroU32>()) as u64,
                &path,
            )?;
            let scope = shape.signature_scope();
            let occurrences = declaration.references().occurrences();
            meter.check_table_entries(occurrences.len() as u64, &path)?;
            for occurrence in occurrences {
                meter.charge_work(1, &path)?;
                let DefaultSourceReferenceRecordV1::Callable(reference) = occurrence.source()
                else {
                    continue;
                };
                charge_path(meter, &path, 4)?;
                let path = path
                    .clone()
                    .field(11)
                    .field(1)
                    .index(u64::from(occurrence.index()))
                    .field(1);
                let context = Context {
                    declaration,
                    occurrence: occurrence.body(),
                    scope: &scope,
                };
                let expected = self
                    .callable_source_domain_at(reference.target().into(), &context, meter, &path)
                    .map_err(|error| {
                        BindingError::target(declaration.key(), occurrence.index(), error)
                    })?;
                let actual = reference.witness().target_domain();
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
                        index: occurrence.index(),
                    });
                }
            }
        }
        Ok(BoundNominalDefaultCallableDomainsV1 { declarations })
    }
}
