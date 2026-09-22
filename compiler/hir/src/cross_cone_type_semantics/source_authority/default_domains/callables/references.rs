use super::*;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn reference_domain(
        &self,
        reference: &DefaultCallableReferenceV1,
        context: &Context<'_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let target = match reference.target() {
            DefaultCallableReferenceTargetV1::Named(callee)
            | DefaultCallableReferenceTargetV1::BoundExtension { callee, .. } => {
                View::Callable(callee)
            }
            DefaultCallableReferenceTargetV1::Local {
                declaration,
                callee,
            } => {
                if callee.declaration() != local_declaration(*declaration)? {
                    return Err(Error::LocalReference(*declaration));
                }
                View::LocalFunction(*declaration)
            }
            DefaultCallableReferenceTargetV1::BoundMember { callee, .. } => match callee {
                DefaultMethodCalleeV1::Callable(callee) => View::Callable(callee),
                DefaultMethodCalleeV1::Bound(bound) => View::Bound(bound),
                DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                    return self.equality_domain(owner_type, context.scope, meter, path);
                }
            },
        };
        let declaration = routes::declaration(target)?;
        if matches!(declaration, Declaration::Generated(_)) {
            // Generated invoke identities are descriptors, not named source targets.
            return Err(Error::target(
                DefaultSourceTargetSubjectError::CallableRole(declaration),
            ));
        }
        self.callable_source_domain_at(target, context, meter, path)
    }
}
