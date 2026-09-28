//! Resolve a declaration-level slot at its concrete receiver application.

use super::*;
use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableOdrMemberId, DefinitionOwnerAtom,
    DispatchDeclarationOwner, OdrGroupId, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole,
    PersistentCallableApplicationId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
    SpecializationKey, StrongCallableDefinitionOwner,
};

pub fn dispatch_declaration_target(
    identities: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    declaration: DispatchDeclarationOwner,
    receiver: PersistentExactTypeId,
) -> Result<CallableDefinitionOwner, MirCallableBridgeError> {
    let (source, strong) = match declaration {
        DispatchDeclarationOwner::Function(id) => (
            identities.canonical_key::<_, SourceDeclarationKey>(id)?,
            StrongCallableDefinitionOwner::Function(id),
        ),
        DispatchDeclarationOwner::Accessor(id) => {
            let accessor = identities.canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(MirCallableBridgeError::InvalidDispatchDeclaration);
            };
            (
                identities.canonical_key::<_, SourceDeclarationKey>(property)?,
                StrongCallableDefinitionOwner::PropertyAccessor(id),
            )
        }
    };
    let generic = match source.owners().owners().last() {
        Some(DefinitionOwnerAtom::Type(_)) => return Ok(strong.into()),
        Some(DefinitionOwnerAtom::GenericType(id)) => *id,
        _ => return Err(MirCallableBridgeError::InvalidDispatchDeclaration),
    };
    let mut pending = vec![receiver];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(exact) = pending.pop() {
        if !seen.insert(exact) {
            continue;
        }
        let key = identities.canonical_key::<_, ExactTypeKey>(exact)?;
        if matches!(key.as_ref(), ExactTypeKey::NominalApplication { origin, .. } if *origin == generic)
        {
            let owner = CallableInstantiationOwner::ExactNominalOwner(exact);
            let application = match declaration {
                DispatchDeclarationOwner::Function(id) => {
                    CallableApplicationKey::for_function(id, owner)
                }
                DispatchDeclarationOwner::Accessor(id) => {
                    CallableApplicationKey::for_accessor(id, owner)
                }
            };
            let id = PersistentCallableApplicationId::from_key(&application)?;
            let group = OdrGroupId::from_key(&SpecializationKey::Callable { application })?;
            let key = OdrMemberKey::new(
                group,
                OdrMemberRole::CallableBody,
                OdrMemberDiscriminator::CallableApplication(id),
            )
            .map_err(MirCallableBridgeError::OdrMember)?;
            return CallableOdrMemberId::from_key(&key)
                .map(CallableDefinitionOwner::Odr)
                .map_err(MirCallableBridgeError::OdrMember);
        }
        let record = types
            .get(exact)
            .ok_or(MirCallableBridgeError::MissingType { exact })?;
        pending.extend(record.base_and_interfaces().interfaces.iter().copied());
        if let MirBaseClassV1::Base(base) = record.base_and_interfaces().base {
            pending.push(base);
        }
    }
    Err(MirCallableBridgeError::InvalidDispatchDeclaration)
}
