use std::collections::BTreeMap;

use scoop_hir::concrete::ExecutableExpressionPosition;
use scoop_identity::{
    CallableApplicationKey, CallableMaterializationContext, CallableOwner, CallableTemplateOrigin,
    CallableTemplateOwner, ExactCallableSignature, OdrMemberDiscriminator, OdrMemberKey,
    OdrMemberRole, PersistentCallableApplicationId, ValidatedIdentityGraph,
};
use scoop_mir::{CallableSignatureSubject, CanonicalMirFoundation, StrongCallableBridgeSurfaceV1};

use crate::CrossConeMirClosureRelationError as Error;

pub(super) struct ApplicationSignature<'a> {
    pub(super) origin: CallableTemplateOrigin,
    pub(super) signature: &'a ExactCallableSignature,
}

pub(super) fn signatures<'a>(
    foundation: &'a CanonicalMirFoundation,
    identities: &ValidatedIdentityGraph,
) -> Result<BTreeMap<PersistentCallableApplicationId, ApplicationSignature<'a>>, Error> {
    let mut applications = BTreeMap::new();
    for signature in foundation.callable_signatures() {
        let CallableSignatureSubject::Odr(member) = signature.subject() else {
            continue;
        };
        if member.role() != OdrMemberRole::CallableBody {
            continue;
        }
        let key = identities
            .canonical_key::<_, OdrMemberKey>(member.member())
            .map_err(|source| Error::CallIdentity(Box::new(source)))?;
        let OdrMemberDiscriminator::CallableApplication(application) = key.discriminator() else {
            continue;
        };
        let origin = identities
            .canonical_key::<_, CallableApplicationKey>(*application)
            .map_err(|source| Error::CallIdentity(Box::new(source)))?
            .origin();
        let entry = ApplicationSignature {
            origin,
            signature: signature.signature(),
        };
        if applications.insert(*application, entry).is_some() {
            return Err(Error::DuplicateMirApplication {
                application: *application,
            });
        }
    }
    Ok(applications)
}

pub(super) fn validate_root(
    position: ExecutableExpressionPosition,
    strong: &StrongCallableBridgeSurfaceV1,
    applications: &BTreeMap<PersistentCallableApplicationId, ApplicationSignature<'_>>,
) -> Result<(), Error> {
    let root = position.root;
    let valid = match root.context() {
        CallableMaterializationContext::NoSubstitution => {
            let owner = match root.template() {
                CallableTemplateOwner::Function(id) => CallableOwner::Function(id),
                CallableTemplateOwner::Constructor(id) => CallableOwner::Constructor(id),
                CallableTemplateOwner::Accessor(id) => CallableOwner::Accessor(id),
                CallableTemplateOwner::Generated(id) => CallableOwner::Generated(id),
                CallableTemplateOwner::GenericFunction(_)
                | CallableTemplateOwner::VariantConstructor(_) => {
                    return Err(Error::CallRoot { position });
                }
            };
            strong.get(owner).is_some()
        }
        CallableMaterializationContext::Application(application) => {
            let origin = match root.template() {
                CallableTemplateOwner::Function(id) => CallableTemplateOrigin::Function(id),
                CallableTemplateOwner::GenericFunction(id) => {
                    CallableTemplateOrigin::GenericFunction(id)
                }
                CallableTemplateOwner::Constructor(id) => CallableTemplateOrigin::Constructor(id),
                CallableTemplateOwner::Accessor(id) => CallableTemplateOrigin::Accessor(id),
                CallableTemplateOwner::VariantConstructor(id) => {
                    CallableTemplateOrigin::VariantConstructor(id)
                }
                CallableTemplateOwner::Generated(_) => return Err(Error::CallRoot { position }),
            };
            applications
                .get(&application)
                .is_some_and(|entry| entry.origin == origin)
        }
        CallableMaterializationContext::InitializationApplication(_) => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::CallRoot { position })
    }
}
