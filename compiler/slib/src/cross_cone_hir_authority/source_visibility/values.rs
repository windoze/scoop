use super::*;
use scoop_hir::{
    DefaultFieldRefV1, DefaultSourceFieldAccessSubjectV1, DefaultSourceIndirectTargetV1,
    DefaultSourceValueTargetV1 as Target, DefaultTargetIdentityQueriesV1, SourceAccessDomainV1,
};

use crate::cross_cone_hir_authority::CrossConeHirDefaultValueAccessError as ValueError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(in crate::cross_cone_hir_authority) fn source_value_access_domain(
        &mut self,
        target: Target<'_>,
    ) -> Result<SourceAccessDomainV1, ValueError> {
        if matches!(target, Target::Field(DefaultFieldRefV1::Tuple { .. })) {
            return Ok(SourceAccessDomainV1::universal());
        }
        let provider = target.source_provider(self.current, self.identities)?;

        let foundation = if provider == self.current {
            self.current_foundation
        } else {
            self.dependencies
                .iter()
                .find(|entry| entry.identity == provider)
                .map(|entry| entry.foundation)
                .ok_or(Error::UnreachableProvider { origin: provider })?
        };
        let query = DefaultTargetIdentityQueriesV1::new(provider, foundation);
        let subject = match target {
            Target::Constructor(target) => query.default_constructor_access_subject(target)?,
            Target::Global(id) => query.default_global_access_subject(id)?,
            Target::Singleton(id) => query
                .default_indirect_access_subject(DefaultSourceIndirectTargetV1::Singleton(id))?,
            Target::Field(target) => match query.default_field_access_subject(target)? {
                DefaultSourceFieldAccessSubjectV1::Declaration(subject) => subject,
                DefaultSourceFieldAccessSubjectV1::TupleElement { .. } => {
                    return Ok(SourceAccessDomainV1::universal());
                }
            },
        };
        let key = query.source_declaration_key(subject)?;
        self.source_declaration_access_domain(subject, key)
            .map_err(Into::into)
    }
}
