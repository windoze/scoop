use super::*;
use scoop_hir::{
    DefaultCallableDeclarationV1 as Callable, DefaultCallableReferenceTargetViewV1 as View,
    DefaultNestedCallableIdentityV1 as Identity, DefaultSourceCallableAccessSubjectV1 as Access,
    DefaultSourceNestedCallableDescriptorV1 as Descriptor,
    DefaultSourceTargetSubjectError as TargetError, DefaultTargetIdentityQueriesV1,
};

impl Closure<'_, '_> {
    pub(super) fn default_callable(
        &mut self,
        target: View<'_>,
        nested: &Nested<'_>,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        path: &WirePath,
    ) -> Result<(), Error> {
        self.world.meter.charge_work(1, path)?;
        if let View::DerivedEquality(owner) = target {
            return self.signature(owner);
        }
        let declaration = target.source_declaration()?;
        let provider = if let Callable::Generated(id) = declaration {
            let source = nested.attached_to(occurrence, self.world.meter, path)?;
            let actual = match source.descriptor().identity() {
                Identity::Lambda(id)
                | Identity::AnonymousFunction(id)
                | Identity::CallableReference(id) => id,
                Identity::LocalFunction(_) => {
                    return Err(TargetError::CallableRole(declaration).into());
                }
            };
            if actual != id {
                return Err(TargetError::CallableRole(declaration).into());
            }
            source.definition_origin().origin().source().cone()
        } else {
            declaration.source_provider(self.world.identities, self.world.meter, path)?
        };
        self.world
            .meter
            .charge_work(self.world.dependencies.len() as u64 + 1, path)?;
        let foundation = if provider == self.world.current {
            self.world.current_foundation
        } else {
            self.world
                .dependencies
                .iter()
                .find(|entry| entry.identity == provider)
                .map(|entry| entry.foundation)
                .ok_or(CrossConeHirNominalAuthorityError::UnreachableProvider {
                    origin: provider,
                })?
        };
        let query =
            DefaultTargetIdentityQueriesV1::new(provider, foundation, self.world.identities);
        match query.default_callable_access_subject_view(target, self.world.meter)? {
            Access::Declaration(_) => self.callable(match declaration {
                Callable::Function(id) => CallableTemplateOrigin::Function(id),
                Callable::GenericFunction(id) => CallableTemplateOrigin::GenericFunction(id),
                Callable::PropertyAccessor(id) => CallableTemplateOrigin::Accessor(id),
                Callable::Generated(_) => return Err(TargetError::CallableRole(declaration).into()),
            }),
            Access::Nested(Identity::LocalFunction(id)) => {
                nested.require_local_declaration(id, self.world.meter, path)?;
                Ok(())
            }
            Access::Nested(identity) => {
                let source = nested.attached_to(occurrence, self.world.meter, path)?;
                if source.descriptor().identity() != identity {
                    return Err(TargetError::NestedRole(identity).into());
                }
                match source.descriptor() {
                    Descriptor::Lambda(_) | Descriptor::AnonymousFunction(_) => Ok(()),
                    Descriptor::CallableReference(reference) => {
                        // This query rejects another generated named target.
                        // Only the attached descriptor can introduce this edge.
                        self.default_callable(
                            reference.source_access_target()?,
                            nested,
                            occurrence,
                            path,
                        )
                    }
                    Descriptor::LocalFunction(_) => Err(TargetError::NestedRole(identity).into()),
                }
            }
            Access::DerivedEquality { owner_type } => self.signature(owner_type),
        }
    }
}
