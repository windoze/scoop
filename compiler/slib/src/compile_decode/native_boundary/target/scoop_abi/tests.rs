use super::*;
use scoop_hir::{
    CanonicalHirFoundation, DecodedHirFoundation, NativeBoundaryCLayoutPolicy,
    NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    Effect, PackagePath, PendingIdentityValidation, PersistentTypeId, ScoopAbiArgument,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode};

mod support;
use support::*;

#[test]
fn abi_replay_borrows_exact_types_from_mixed_providers_without_core_selection() {
    let current = Fixture::empty();
    let reference = Fixture::nominal(ConeIdentity::CORE, true);
    let value = Fixture::nominal(ConeIdentity::SINGLE_FILE, false);
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(reference.exact()),
        vec![value.exact()],
        reference.exact(),
    );
    let dependencies = [reference.borrow(), value.borrow()];
    let replay = replay(&current, &dependencies, &signature).unwrap();
    assert!(
        matches!(replay.arguments(), [scoop_identity::ScoopAbiArgument::Direct(value), scoop_identity::ScoopAbiArgument::ElidedZst(zst)] if value.byte_size() == 8 && zst.byte_size() == 0)
    );
    assert!(matches!(replay.result(), ScoopAbiReturn::Direct(storage) if storage.byte_size() == 8));
    let reversed = [value.borrow(), reference.borrow()];
    assert_eq!(
        replay,
        self::replay(&current, &reversed, &signature).unwrap()
    );
    assert!(self::replay(&current, &[reference.borrow()], &signature).is_err());
}

fn replay(
    current: &Fixture,
    dependencies: &[AbiReplayDependency<'_>],
    signature: &ExactCallableSignature,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    replay_canonical_scoop_abi_parts(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        current.borrow(),
        dependencies,
        signature,
        GcEffect::Managed,
    )
}

#[test]
fn ordinary_callable_abi_replays_intrinsics_from_mixed_actual_providers() {
    let current = Fixture::empty();
    let integer = Fixture::intrinsic(
        ConeIdentity::SINGLE_FILE,
        scoop_hir::IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_64),
    );
    let boolean = Fixture::intrinsic(ConeIdentity::CORE, scoop_hir::IntrinsicTypeKind::Boolean);
    let string = Fixture::intrinsic(ConeIdentity::CORE, scoop_hir::IntrinsicTypeKind::String);
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        None,
        vec![integer.exact(), boolean.exact()],
        string.exact(),
    );
    let actual = replay(
        &current,
        &[integer.borrow(), boolean.borrow(), string.borrow()],
        &signature,
    )
    .unwrap();
    assert!(
        matches!(actual.arguments(), [ScoopAbiArgument::Direct(integer), ScoopAbiArgument::Direct(boolean)]
        if integer.byte_size() == 8 && boolean.byte_size() == 1)
    );
    assert!(matches!(actual.result(), ScoopAbiReturn::Direct(storage) if storage.byte_size() == 8));
    assert!(
        matches!(replay(&current, &[boolean.borrow(), string.borrow()], &signature),
        Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::MissingExactType { exact })) if exact == integer.exact())
    );
}

mod intrinsic_sources;
mod shared_sources;

#[test]
fn interface_abi_uses_two_parts_from_native_or_shared_declarations() {
    for native in [true, false] {
        let interface = Fixture::from_shape(
            ConeIdentity::CORE,
            SourceNominalKind::Interface,
            NativeBoundaryNominalShape::Reference,
        );
        let interface = if native {
            interface
        } else {
            interface.without_native_witness()
        };
        let current = Fixture::empty();
        let signature = ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![interface.exact()],
            interface.exact(),
        );
        for target in [
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            scoop_lir::LirTargetProfile::LINUX_X86_64_GNU,
            scoop_lir::LirTargetProfile::LINUX_X86_64_MUSL,
        ] {
            let abi = replay_canonical_scoop_abi_parts(
                target,
                current.borrow(),
                &[interface.borrow()],
                &signature,
                GcEffect::Managed,
            )
            .unwrap();
            assert!(
                matches!(abi.arguments(), [ScoopAbiArgument::DirectParts(value)]
                if value.byte_size() == 16 && value.alignment().get() == 8)
            );
            assert!(matches!(abi.result(), ScoopAbiReturn::DirectParts(value)
                if value.byte_size() == 16 && value.alignment().get() == 8));
        }
    }
}
