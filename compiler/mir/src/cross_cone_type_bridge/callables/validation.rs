use super::*;
use scoop_identity::{
    AccessorRole, DefinitionOwnerAtom, DispatchSlotKey, ExactTypeKey, OptionalExactOwner,
    PropertyAccessorKey, SourceDeclarationKey,
};

mod generated;

#[derive(Debug)]
pub enum MirCallableBridgeError {
    Reference(IdentityReferenceError),
    ExactSignature(scoop_identity::ExactCallableSignatureResolutionError<IdentityReferenceError>),
    Type(MirTypeBridgeError),
    Resource(WireError),
    GeneratedRoleReference(
        Box<scoop_identity::GeneratedCallableResolutionError<IdentityReferenceError>>,
    ),
    OriginMismatch,
    GeneratedRoleMismatch,
    GeneratedExecutionGate {
        callable: PersistentGeneratedCallableId,
    },
    MissingImplementation {
        implementation: StrongCallableDefinitionOwner,
    },
    FoundationSignatureMismatch {
        implementation: StrongCallableDefinitionOwner,
    },
    MissingType {
        exact: PersistentExactTypeId,
    },
    RoleMismatch,
    SignatureMismatch,
    SuspendSynchronousRole,
    NoGcContainsReferences {
        exact: PersistentExactTypeId,
    },
    ConstructorOwnerMismatch,
    InvalidAccessorShape,
    InvalidInitializationUnit,
    InvalidAdjustTarget,
    InvalidTrapDeclaration,
    DuplicateImplementation {
        implementation: StrongCallableDefinitionOwner,
    },
    NonCanonicalBindingOrder {
        index: usize,
    },
}
impl From<IdentityReferenceError> for MirCallableBridgeError {
    fn from(value: IdentityReferenceError) -> Self {
        Self::Reference(value)
    }
}
impl std::fmt::Display for MirCallableBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR callable bridge: {self:?}")
    }
}
impl std::error::Error for MirCallableBridgeError {}

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn validate(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
    ) -> Result<(), MirCallableBridgeError> {
        if binding.origin.implementation() != binding.implementation {
            return Err(MirCallableBridgeError::OriginMismatch);
        }
        self.validate_origin(&binding.origin)?;
        for signature in [&binding.semantic, &binding.lowered] {
            for exact in signature
                .exact()
                .receiver()
                .into_option()
                .into_iter()
                .chain(signature.exact().parameters().iter().copied())
                .chain([signature.exact().result()])
            {
                let facts = self.type_export(exact)?.facts();
                if signature.gc_effect() == crate::GcEffect::NoGc
                    && facts.gc() != MirGcKindV1::GcFree
                {
                    return Err(MirCallableBridgeError::NoGcContainsReferences { exact });
                }
            }
        }
        if self.foundation_signature(binding.implementation)? != binding.lowered.exact() {
            return Err(MirCallableBridgeError::FoundationSignatureMismatch {
                implementation: binding.implementation,
            });
        }
        let semantic = binding.semantic.exact();
        let lowered = binding.lowered.exact();
        if matches!(
            binding.role,
            MirCallableLoweringRoleV1::ClassInitializer { .. }
                | MirCallableLoweringRoleV1::ValueConstructor { .. }
                | MirCallableLoweringRoleV1::Accessor
                | MirCallableLoweringRoleV1::ObjectEnsure { .. }
                | MirCallableLoweringRoleV1::ObjectInitializer { .. }
                | MirCallableLoweringRoleV1::DerivedEquality { .. }
        ) && semantic.effect() != scoop_identity::Effect::Ordinary
        {
            return Err(MirCallableBridgeError::SuspendSynchronousRole);
        }
        let wraps_no_gc = matches!(
            binding.role,
            MirCallableLoweringRoleV1::DispatchAdjust { .. }
                | MirCallableLoweringRoleV1::BoxingAdjust { .. }
        ) && binding.semantic.gc_effect() == crate::GcEffect::NoGc
            && binding.lowered.gc_effect() == crate::GcEffect::Managed;
        if semantic.effect() != lowered.effect()
            || (binding.semantic.gc_effect() != binding.lowered.gc_effect() && !wraps_no_gc)
        {
            return Err(MirCallableBridgeError::SignatureMismatch);
        }
        match (&binding.origin, binding.role) {
            (MirCallableOriginV1::Function(_), MirCallableLoweringRoleV1::Ordinary) => {
                self.same_signatures(binding)
            }
            (MirCallableOriginV1::Accessor(accessor), MirCallableLoweringRoleV1::Accessor) => {
                self.same_signatures(binding)?;
                let key = self
                    .identities
                    .canonical_key::<_, PropertyAccessorKey>(*accessor)?;
                let valid = match key.role() {
                    AccessorRole::Getter => semantic.parameters().is_empty(),
                    AccessorRole::Setter => {
                        semantic.parameters().len() == 1 && self.is_unit(semantic.result())?
                    }
                };
                if valid {
                    Ok(())
                } else {
                    Err(MirCallableBridgeError::InvalidAccessorShape)
                }
            }
            (
                MirCallableOriginV1::Constructor(constructor),
                MirCallableLoweringRoleV1::ClassInitializer { owner },
            ) => {
                self.constructor_owner(*constructor, owner)?;
                if !matches!(
                    self.type_export(owner)?.representation(),
                    MirTypeRepresentationV1::Class { .. }
                        | MirTypeRepresentationV1::ObjectBacking { .. }
                ) || semantic.receiver().is_present()
                    || semantic.result() != owner
                    || lowered.receiver() != OptionalExactOwner::Present(owner)
                    || semantic.parameters() != lowered.parameters()
                    || !self.is_unit(lowered.result())?
                {
                    return Err(MirCallableBridgeError::SignatureMismatch);
                }
                Ok(())
            }
            (
                MirCallableOriginV1::Constructor(constructor),
                MirCallableLoweringRoleV1::ValueConstructor { owner },
            ) => {
                self.constructor_owner(*constructor, owner)?;
                self.same_signatures(binding)?;
                if !matches!(
                    self.type_export(owner)?.representation(),
                    MirTypeRepresentationV1::Struct { .. }
                ) || semantic.receiver().is_present()
                    || semantic.result() != owner
                {
                    return Err(MirCallableBridgeError::SignatureMismatch);
                }
                Ok(())
            }
            (_, MirCallableLoweringRoleV1::PureVirtualTrap { slot }) => {
                self.same_signatures(binding)?;
                let key = self.identities.canonical_key::<_, DispatchSlotKey>(slot)?;
                if declaration_implementation(key.owner()) != binding.implementation {
                    return Err(MirCallableBridgeError::InvalidTrapDeclaration);
                }
                Ok(())
            }
            (MirCallableOriginV1::Generated { role, .. }, _) => {
                self.validate_generated(binding, role)
            }
            _ => Err(MirCallableBridgeError::RoleMismatch),
        }
    }

    fn validate_origin(&self, origin: &MirCallableOriginV1) -> Result<(), MirCallableBridgeError> {
        match origin {
            MirCallableOriginV1::Function(id) => {
                self.identities
                    .canonical_key::<_, SourceDeclarationKey>(*id)?;
            }
            MirCallableOriginV1::Constructor(id) => {
                self.identities
                    .canonical_key::<_, SourceDeclarationKey>(*id)?;
            }
            MirCallableOriginV1::Accessor(id) => {
                self.identities
                    .canonical_key::<_, PropertyAccessorKey>(*id)?;
            }
            MirCallableOriginV1::Generated { callable, role } => {
                if self
                    .identities
                    .canonical_key::<_, GeneratedCallableKey>(*callable)?
                    .as_ref()
                    != role
                {
                    return Err(MirCallableBridgeError::GeneratedRoleMismatch);
                }
            }
        }
        Ok(())
    }
    fn same_signatures(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
    ) -> Result<(), MirCallableBridgeError> {
        if binding.semantic == binding.lowered {
            Ok(())
        } else {
            Err(MirCallableBridgeError::SignatureMismatch)
        }
    }
    fn type_export(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, MirCallableBridgeError> {
        self.types
            .get(exact)
            .ok_or(MirCallableBridgeError::MissingType { exact })
    }
    fn is_unit(&self, exact: PersistentExactTypeId) -> Result<bool, MirCallableBridgeError> {
        Ok(matches!(
            self.type_export(exact)?.representation(),
            MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit)
        ))
    }
    fn foundation_signature(
        &self,
        implementation: StrongCallableDefinitionOwner,
    ) -> Result<&ExactCallableSignature, MirCallableBridgeError> {
        let subject = crate::CallableSignatureSubject::strong(implementation.callable_owner());
        let entries = self.foundation.as_canonical().callable_signatures();
        entries
            .binary_search_by(|entry| entry.subject().compare_sort_key(subject))
            .ok()
            .map(|index| entries[index].signature())
            .ok_or(MirCallableBridgeError::MissingImplementation { implementation })
    }
    fn constructor_owner(
        &self,
        constructor: PersistentConstructorId,
        owner: PersistentExactTypeId,
    ) -> Result<(), MirCallableBridgeError> {
        let key = self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(constructor)?;
        let exact = self.identities.canonical_key::<_, ExactTypeKey>(owner)?;
        if !matches!((key.owners().owners().last(), exact.as_ref()),
            (Some(DefinitionOwnerAtom::Type(declared)), ExactTypeKey::Nominal(actual)) if declared == actual)
        {
            return Err(MirCallableBridgeError::ConstructorOwnerMismatch);
        }
        Ok(())
    }
}
pub(super) fn declaration_implementation(
    declaration: DispatchDeclarationOwner,
) -> StrongCallableDefinitionOwner {
    match declaration {
        DispatchDeclarationOwner::Function(id) => StrongCallableDefinitionOwner::Function(id),
        DispatchDeclarationOwner::Accessor(id) => {
            StrongCallableDefinitionOwner::PropertyAccessor(id)
        }
    }
}
