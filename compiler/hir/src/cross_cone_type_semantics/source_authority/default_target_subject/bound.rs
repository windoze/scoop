use super::*;

impl BoundTypeFoundationSourcesV1<'_> {
    fn default_identity_queries(&self) -> DefaultTargetIdentityQueriesV1<'_> {
        DefaultTargetIdentityQueriesV1::new(
            self.source().entries().provider,
            self.foundation,
            self.identities,
        )
    }

    pub fn default_indirect_access_subject(
        &self,
        target: DefaultSourceIndirectTargetV1,
    ) -> Result<Subject, Error> {
        self.default_identity_queries()
            .default_indirect_access_subject(target)
    }

    pub fn default_field_access_subject(
        &self,
        target: &DefaultFieldRefV1,
    ) -> Result<DefaultSourceFieldAccessSubjectV1, Error> {
        self.default_identity_queries()
            .default_field_access_subject(target)
    }

    pub fn default_constructor_access_subject(
        &self,
        target: &DefaultConstructorRefV1,
    ) -> Result<Subject, Error> {
        self.default_identity_queries()
            .default_constructor_access_subject(target)
    }

    pub fn default_global_access_subject(
        &self,
        id: PersistentPropertyId,
    ) -> Result<Subject, Error> {
        self.default_identity_queries()
            .default_global_access_subject(id)
    }

    pub fn default_callable_access_subject<'t>(
        &self,
        target: &'t ExportDefaultCallableTargetV1,
    ) -> Result<DefaultSourceCallableAccessSubjectV1<'t>, Error> {
        self.default_identity_queries()
            .default_callable_access_subject(target)
    }

    pub(crate) fn default_callable_access_subject_view<'t>(
        &self,
        target: DefaultCallableReferenceTargetViewV1<'t>,
    ) -> Result<DefaultSourceCallableAccessSubjectV1<'t>, Error> {
        self.default_identity_queries()
            .default_callable_access_subject_view(target)
    }
}
