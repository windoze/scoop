use super::*;
use crate::cross_cone_hir_authority::CrossConeHirNominalAuthorityError as DeclarationError;
use scoop_hir::{
    DefaultCallableDeclarationV1 as Declaration, DefaultNestedCallableIdentityV1 as Identity,
    DefaultSourceCallableAccessSubjectV1 as Access,
    DefaultSourceNestedCallableDescriptorV1 as Descriptor, DefaultSourceTargetSubjectError,
    DefaultTargetIdentityQueriesV1, SourceAccessDomainV1,
};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn source_callable_access_domain(
        &mut self,
        target: View<'_>,
        context: &Context<'_, '_>,
        protocols: &CoreBootstrapInterfaceSectionV1,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, Error> {
        self.meter.charge_nodes(1, path)?;
        self.meter.charge_work(1, path)?;
        if let View::DerivedEquality(owner) = target {
            return self.source_equality_access_domain(owner, protocols, path);
        }
        let declaration = target.source_declaration()?;
        let provider = if let Declaration::Generated(id) = declaration {
            let source = context
                .nested
                .attached_to(context.occurrence, self.meter, path)?;
            let actual = match source.descriptor().identity() {
                Identity::Lambda(id)
                | Identity::AnonymousFunction(id)
                | Identity::CallableReference(id) => id,
                Identity::LocalFunction(_) => {
                    return Err(DefaultSourceTargetSubjectError::CallableRole(declaration).into());
                }
            };
            if id != actual {
                return Err(DefaultSourceTargetSubjectError::CallableRole(declaration).into());
            }
            source.definition_origin().origin().source().cone()
        } else {
            declaration.source_provider(self.identities, self.meter, path)?
        };
        self.meter
            .charge_work(self.dependencies.len() as u64 + 1, path)?;
        let foundation = if provider == self.current {
            self.current_foundation
        } else {
            self.dependencies
                .iter()
                .find(|entry| entry.identity == provider)
                .map(|entry| entry.foundation)
                .ok_or(DeclarationError::UnreachableProvider { origin: provider })?
        };
        let query = DefaultTargetIdentityQueriesV1::new(provider, foundation, self.identities);
        match query.default_callable_access_subject_view(target, self.meter)? {
            Access::Declaration(subject) => {
                let key = query.source_declaration_key(subject, self.meter)?;
                self.source_declaration_access_domain(subject, key, path)
                    .map_err(Into::into)
            }
            Access::Nested(Identity::LocalFunction(declaration)) => {
                context
                    .nested
                    .require_local_declaration(declaration, self.meter, path)?;
                Ok(SourceAccessDomainV1::universal())
            }
            Access::Nested(identity) => {
                let source = context
                    .nested
                    .attached_to(context.occurrence, self.meter, path)?;
                if source.descriptor().identity() != identity {
                    return Err(DefaultSourceTargetSubjectError::NestedRole(identity).into());
                }
                match source.descriptor() {
                    Descriptor::Lambda(_) | Descriptor::AnonymousFunction(_) => {
                        Ok(SourceAccessDomainV1::universal())
                    }
                    Descriptor::CallableReference(reference) => {
                        // The shared target query rejects generated named callees,
                        // so this one-level source indirection cannot recurse again.
                        let target = reference.source_access_target()?;
                        self.source_callable_access_domain(target, context, protocols, path)
                    }
                    Descriptor::LocalFunction(_) => {
                        Err(DefaultSourceTargetSubjectError::NestedRole(identity).into())
                    }
                }
            }
            Access::DerivedEquality { owner_type } => {
                self.source_equality_access_domain(owner_type, protocols, path)
            }
        }
    }
}
