use super::*;
use scoop_identity::PropertyOwner;

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn validate_trap(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        slot: PersistentDispatchSlotId,
    ) -> Result<(), MirCallableBridgeError> {
        let key = self.identities.canonical_key::<_, DispatchSlotKey>(slot)?;
        let root = declaration_implementation(key.owner());
        if root == binding.implementation {
            return Ok(());
        }
        let source = binding.semantic.exact();
        let inherited = self.foundation_signature(root)?;
        if source.effect() != inherited.effect()
            || source.parameters() != inherited.parameters()
            || source.result() != inherited.result()
        {
            return Err(MirCallableBridgeError::InvalidTrapDeclaration);
        }
        let owner = self.trap_receiver(binding.implementation, source)?;
        let root_owner = self.trap_receiver(root, inherited)?;
        if owner == root_owner
            || !matches!(
                self.type_export(root_owner)?.representation(),
                MirTypeRepresentationV1::Class { .. }
            )
            || !matches!(
                self.type_export(owner)?.representation(),
                MirTypeRepresentationV1::Class {
                    kind: MirClassKindV1::Abstract,
                    ..
                }
            )
        {
            return Err(MirCallableBridgeError::InvalidTrapDeclaration);
        }
        // The checked type table bounds this iterative ancestry walk, even
        // before the enclosing section rejects malformed inheritance cycles.
        let mut current = owner;
        for _ in 0..self.types.record_count() {
            let record = self.type_export(current)?;
            if !matches!(
                record.representation(),
                MirTypeRepresentationV1::Class { .. }
            ) {
                return Err(MirCallableBridgeError::InvalidTrapDeclaration);
            }
            let MirBaseClassV1::Base(base) = record.base_and_interfaces().base else {
                return Err(MirCallableBridgeError::InvalidTrapDeclaration);
            };
            if base == root_owner {
                return Ok(());
            }
            current = base;
        }
        Err(MirCallableBridgeError::InvalidTrapDeclaration)
    }

    fn trap_receiver(
        &self,
        declaration: StrongCallableDefinitionOwner,
        signature: &ExactCallableSignature,
    ) -> Result<PersistentExactTypeId, MirCallableBridgeError> {
        let exact = signature
            .receiver()
            .into_option()
            .ok_or(MirCallableBridgeError::InvalidTrapDeclaration)?;
        let record = self.type_export(exact)?;
        let MirTypeOriginV1::SourceNominal(nominal) = *record.origin() else {
            return Err(MirCallableBridgeError::InvalidTrapDeclaration);
        };
        let key = match declaration {
            StrongCallableDefinitionOwner::Function(id) => self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(id)?,
            StrongCallableDefinitionOwner::PropertyAccessor(id) => {
                let accessor = self
                    .identities
                    .canonical_key::<_, PropertyAccessorKey>(id)?;
                let PropertyOwner::Property(property) = accessor.owner() else {
                    return Err(MirCallableBridgeError::InvalidTrapDeclaration);
                };
                self.identities
                    .canonical_key::<_, SourceDeclarationKey>(property)?
            }
            _ => return Err(MirCallableBridgeError::InvalidTrapDeclaration),
        };
        if !matches!(key.owners().owners().last(), Some(DefinitionOwnerAtom::Type(owner)) if *owner == nominal)
        {
            return Err(MirCallableBridgeError::InvalidTrapDeclaration);
        }
        Ok(exact)
    }
}
