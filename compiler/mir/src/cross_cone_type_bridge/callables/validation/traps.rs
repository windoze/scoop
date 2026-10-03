use super::*;
use scoop_identity::PropertyOwner;

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn validate_trap(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        slot: PersistentDispatchSlotId,
    ) -> Result<(), MirCallableBridgeError> {
        let key = self.identities.canonical_key::<_, DispatchSlotKey>(slot)?;
        let receiver = binding
            .semantic
            .exact()
            .receiver()
            .into_option()
            .ok_or(MirCallableBridgeError::InvalidTrapDeclaration)?;
        let root = dispatch_declaration_target(self.identities, self.types, key.owner(), receiver)?;
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
        declaration: CallableDefinitionOwner,
        signature: &ExactCallableSignature,
    ) -> Result<PersistentExactTypeId, MirCallableBridgeError> {
        let exact = signature
            .receiver()
            .into_option()
            .ok_or(MirCallableBridgeError::InvalidTrapDeclaration)?;
        let record = self.type_export(exact)?;
        let declaration = match declaration {
            CallableDefinitionOwner::Odr(member) => {
                let member = self
                    .identities
                    .canonical_key::<_, scoop_identity::OdrMemberKey>(member.member())?;
                let scoop_identity::OdrMemberDiscriminator::CallableApplication(application) =
                    member.discriminator()
                else {
                    return Err(MirCallableBridgeError::InvalidTrapDeclaration);
                };
                let application = self
                    .identities
                    .canonical_key::<_, scoop_identity::CallableApplicationKey>(*application)?;
                let template = match application.origin() {
                    scoop_identity::CallableTemplateOrigin::Function(id) => {
                        DispatchDeclarationOwner::Function(id)
                    }
                    scoop_identity::CallableTemplateOrigin::Accessor(id) => {
                        DispatchDeclarationOwner::Accessor(id)
                    }
                    _ => return Err(MirCallableBridgeError::InvalidTrapDeclaration),
                };
                if dispatch_declaration_target(self.identities, self.types, template, exact)?
                    != declaration
                {
                    return Err(MirCallableBridgeError::InvalidTrapDeclaration);
                }
                return Ok(exact);
            }
            strong => strong,
        };
        let MirTypeOriginV1::SourceNominal(nominal) = *record.origin() else {
            return Err(MirCallableBridgeError::InvalidTrapDeclaration);
        };
        let key = match declaration {
            CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::Function(id)) => self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(id)?,
            CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::PropertyAccessor(
                id,
            )) => {
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
