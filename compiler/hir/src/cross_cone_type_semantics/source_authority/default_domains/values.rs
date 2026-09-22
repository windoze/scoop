//! Target domains for stored values, fields and constructor declarations.
use super::*;
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId};

mod binding;
mod routes;
pub use binding::*;

/// Query inputs, without operation typing, receiver or execution authority.
#[derive(Clone, Copy, Debug)]
pub enum DefaultSourceValueTargetV1<'a> {
    Constructor(&'a DefaultConstructorRefV1),
    Global(PersistentPropertyId),
    Singleton(PersistentObjectValueId),
    Field(&'a DefaultFieldRefV1),
}
type Target<'a> = DefaultSourceValueTargetV1<'a>;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn value_source_domain(
        &self,
        target: Target<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        self.value_source_domain_at(target, meter, &WirePath::root())
    }

    fn value_source_domain_at(
        &self,
        target: Target<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        let provider = self.value_provider(target, meter, path)?;
        let foundation = provider.foundation;
        let subject = match target {
            Target::Constructor(target) => foundation
                .default_constructor_access_subject(target, meter)
                .map_err(Error::target)?,
            Target::Global(target) => foundation
                .default_global_access_subject(target, meter)
                .map_err(Error::target)?,
            Target::Singleton(target) => foundation
                .default_indirect_access_subject(
                    DefaultSourceIndirectTargetV1::Singleton(target),
                    meter,
                )
                .map_err(Error::target)?,
            Target::Field(target) => match foundation
                .default_field_access_subject(target, meter)
                .map_err(Error::target)?
            {
                DefaultSourceFieldAccessSubjectV1::Declaration(subject) => subject,
                DefaultSourceFieldAccessSubjectV1::TupleElement { .. } => {
                    return Ok(DefaultSourceAccessDomainV1::universal());
                }
            },
        };
        provider
            .source_lookup_domain(subject, meter)
            .map_err(Error::domain)
    }
}
