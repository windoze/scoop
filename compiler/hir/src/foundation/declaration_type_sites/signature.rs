use super::Error;
use crate::HirCallableTypePositionV1 as Part;
use scoop_identity::{
    AccessorRole, CallableMaterialization, DefinitionOwnerAtom, DuplicateSignatureKey,
    GeneratedCallableKey, SourceDeclarationKey,
};

pub(super) fn source(
    key: &SourceDeclarationKey,
    root: CallableMaterialization,
    position: Option<Part>,
) -> Result<(), Error> {
    let Some(position) = position else {
        return Ok(());
    };
    let valid = match (key.duplicate_signature(), position) {
        (DuplicateSignatureKey::Function { .. }, Part::Receiver) => has_receiver(key),
        (
            DuplicateSignatureKey::Function { parameters, .. }
            | DuplicateSignatureKey::Constructor { parameters },
            Part::Parameter(index),
        ) => (index as usize) < parameters.len(),
        (
            DuplicateSignatureKey::Function { .. } | DuplicateSignatureKey::Constructor { .. },
            Part::Result,
        ) => true,
        _ => false,
    };
    require(valid, root, position)
}

pub(super) fn accessor(
    property: &SourceDeclarationKey,
    role: AccessorRole,
    root: CallableMaterialization,
    position: Option<Part>,
) -> Result<(), Error> {
    let Some(position) = position else {
        return Ok(());
    };
    let valid = match position {
        Part::Receiver => has_receiver(property),
        Part::Parameter(index) => role == AccessorRole::Setter && index == 0,
        Part::Result => true,
    };
    require(valid, root, position)
}

pub(super) fn generated(
    key: &GeneratedCallableKey,
    root: CallableMaterialization,
    position: Option<Part>,
) -> Result<(), Error> {
    let Some(position) = position else {
        return Ok(());
    };
    let valid = match key {
        GeneratedCallableKey::Initialization { .. }
        | GeneratedCallableKey::ZeroArgumentConstructorAdapter { .. } => position == Part::Result,
        GeneratedCallableKey::DerivedEquality { .. }
        | GeneratedCallableKey::TupleEncoding { .. } => {
            matches!(position, Part::Receiver | Part::Parameter(0) | Part::Result)
        }
        // Other generated keys do not contain a complete source signature.
        // Their positions remain subject to the actual MIR signature join.
        _ => true,
    };
    require(valid, root, position)
}

fn has_receiver(key: &SourceDeclarationKey) -> bool {
    key.duplicate_signature().receiver_is_present()
        || matches!(
            key.owners().owners().last(),
            Some(DefinitionOwnerAtom::Type(_) | DefinitionOwnerAtom::GenericType(_))
        )
}

fn require(valid: bool, root: CallableMaterialization, position: Part) -> Result<(), Error> {
    if valid {
        Ok(())
    } else {
        Err(Error::SignaturePosition(root, position))
    }
}
