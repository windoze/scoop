use super::*;

impl FunctionIdentityBuilder<'_> {
    pub(super) fn derived_identity(
        &self,
        function: hir::FunctionId,
    ) -> Result<hir::HirFunctionIdentity, PersistentFunctionIdentityError> {
        let applications = self
            .lowerer
            .derived_equality_applications
            .iter()
            .filter(|(_, application)| application.function == function)
            .filter_map(
                |(application, value)| match self.types.get(value.owner_ty) {
                    Some(hir::HirTypeIdentity::Exact(owner)) => Some(
                        hir::HirDerivedEqualityFunctionIdentity::new(application, owner.id())
                            .map_err(|error| {
                                self.failure(function, Detail::InvalidIdentity(error))
                            }),
                    ),
                    // The full application and its explicit open type remain in
                    // source HIR; only exact applications own machine identities.
                    Some(hir::HirTypeIdentity::Open(_)) => None,
                    None => Some(Err(self.failure(function, Detail::InvalidFunctionOrigin))),
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        Ok(hir::HirFunctionIdentity::derived_equality(applications))
    }
}
