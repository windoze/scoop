//! Source target domains for complete, artifact-bound callable occurrences.
use super::*;
use DefaultCallableDeclarationV1 as Declaration;
use DefaultCallableReferenceTargetViewV1 as View;
use DefaultNestedCallableIdentityV1 as Nested;
use DefaultSourceCallableAccessSubjectV1 as Access;
use DefaultSourceNestedCallableDescriptorV1 as Descriptor;
use scoop_identity::CallableTemplateOrigin;

mod binding;
mod equality;
mod nested;
mod references;
mod routes;
pub use binding::*;

struct Context<'r, 's> {
    declaration: &'r DefaultSourceDeclaredContractV1<'s>,
    occurrence: DefaultBodyReferenceOccurrenceV1<'s>,
    scope: &'r SignatureBinderScopeV1,
}

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    fn callable_source_domain_at(
        &self,
        target: View<'_>,
        context: &Context<'_, '_>,

        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        if let View::DerivedEquality(owner) = target {
            return self.equality_domain(owner, context.scope, path);
        }
        let provider = self.callable_provider(target, context)?;
        match provider
            .foundation
            .default_callable_access_subject_view(target)
            .map_err(Error::target)?
        {
            Access::Declaration(subject) => provider
                .source_lookup_domain(subject)
                .map_err(Error::domain),
            Access::Nested(Nested::LocalFunction(declaration)) => {
                nested::local(context, declaration)?;
                Ok(DefaultSourceAccessDomainV1::universal())
            }
            Access::Nested(identity) => {
                let source = nested::attached(context)?;
                if source.descriptor().identity() != identity {
                    return Err(Error::NestedOccurrence(identity));
                }
                match source.descriptor() {
                    Descriptor::Lambda(_) | Descriptor::AnonymousFunction(_) => {
                        Ok(DefaultSourceAccessDomainV1::universal())
                    }
                    Descriptor::CallableReference(reference) => {
                        self.reference_domain(reference, context, path)
                    }
                    Descriptor::LocalFunction(_) => Err(Error::NestedOccurrence(identity)),
                }
            }
            Access::DerivedEquality { owner_type } => {
                self.equality_domain(owner_type, context.scope, path)
            }
        }
    }
}
