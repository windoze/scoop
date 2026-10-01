use std::collections::{BTreeMap, BTreeSet};

use scoop_hir::concrete::ExecutableExpressionPosition;
use scoop_identity::{
    CallableApplicationKey, CallableMaterializationContext, CallableOwner, CallableTemplateOrigin,
    CallableTemplateOwner, ExactCallableSignature, InitializationUnitKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PersistentCallableApplicationId,
    PersistentGeneratedCallableId, ValidatedIdentityGraph,
};
use scoop_mir::{CallableSignatureSubject, CanonicalMirFoundation, StrongCallableBridgeSurfaceV1};

use crate::CrossConeMirClosureRelationError as Error;

pub(super) struct ApplicationSignature<'a> {
    pub(super) origin: CallableTemplateOrigin,
    pub(super) signature: &'a ExactCallableSignature,
    group: OdrGroupId,
}

pub(super) struct Signatures<'a> {
    pub(super) applications: BTreeMap<PersistentCallableApplicationId, ApplicationSignature<'a>>,
    generated: BTreeSet<(OdrGroupId, PersistentGeneratedCallableId)>,
}

pub(super) fn signatures<'a>(
    foundation: &'a CanonicalMirFoundation,
    identities: &ValidatedIdentityGraph,
    callables: Option<&'a scoop_mir::CanonicalMirCallableBindingsV1>,
) -> Result<Signatures<'a>, Error> {
    let mut applications = BTreeMap::new();
    let mut generated = BTreeSet::new();
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
        let application = match key.discriminator() {
            OdrMemberDiscriminator::CallableApplication(application) => *application,
            OdrMemberDiscriminator::GeneratedCallable(callable) => {
                generated.insert((key.group(), *callable));
                continue;
            }
            _ => continue,
        };
        let origin = identities
            .canonical_key::<_, CallableApplicationKey>(application)
            .map_err(|source| Error::CallIdentity(Box::new(source)))?
            .origin();
        let entry = ApplicationSignature {
            origin,
            // Shared bindings retain the source signature; the foundation
            // records the physical continuation/step ABI of suspend bodies.
            signature: callables
                .and_then(|callables| {
                    callables.get(scoop_identity::CallableDefinitionOwner::Odr(member))
                })
                .map_or(signature.signature(), |binding| {
                    binding.semantic_signature().exact()
                }),
            group: key.group(),
        };
        if applications.insert(application, entry).is_some() {
            return Err(Error::DuplicateMirApplication { application });
        }
    }
    Ok(Signatures {
        applications,
        generated,
    })
}

pub(super) fn validate_root(
    position: ExecutableExpressionPosition,
    strong: &StrongCallableBridgeSurfaceV1,
    signatures: &Signatures<'_>,
    identities: &ValidatedIdentityGraph,
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
            if let CallableTemplateOwner::Generated(callable) = root.template() {
                return signatures
                    .applications
                    .get(&application)
                    .is_some_and(|entry| signatures.generated.contains(&(entry.group, callable)))
                    .then_some(())
                    .ok_or(Error::CallRoot { position });
            }
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
            signatures
                .applications
                .get(&application)
                .is_some_and(|entry| entry.origin == origin)
        }
        CallableMaterializationContext::InitializationApplication(unit) => {
            let CallableTemplateOwner::Generated(callable) = root.template() else {
                return Err(Error::CallRoot { position });
            };
            let unit = identities
                .canonical_key::<_, InitializationUnitKey>(unit)
                .map_err(|source| Error::CallIdentity(Box::new(source)))?;
            let key = unit
                .specialization_key()
                .ok_or(Error::CallRoot { position })?;
            let group = OdrGroupId::from_key(&key).map_err(|_| Error::CallRoot { position })?;
            signatures.generated.contains(&(group, callable))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(Error::CallRoot { position })
    }
}
