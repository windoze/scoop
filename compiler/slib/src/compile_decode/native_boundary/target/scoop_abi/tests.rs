use super::*;
use scoop_hir::{
    CanonicalHirFoundation, DecodedHirFoundation, NativeBoundaryCLayoutPolicy,
    NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    Effect, PackagePath, PendingIdentityValidation, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

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
    let replay = replay(&current, &dependencies, &signature, &mut meter()).unwrap();
    assert!(
        matches!(replay.arguments(), [scoop_identity::ScoopAbiArgument::Direct(value), scoop_identity::ScoopAbiArgument::ElidedZst(zst)] if value.byte_size() == 8 && zst.byte_size() == 0)
    );
    assert!(matches!(replay.result(), ScoopAbiReturn::Direct(storage) if storage.byte_size() == 8));
    let reversed = [value.borrow(), reference.borrow()];
    assert_eq!(
        replay,
        self::replay(&current, &reversed, &signature, &mut meter()).unwrap()
    );
    assert!(self::replay(&current, &[reference.borrow()], &signature, &mut meter()).is_err());
}

#[test]
fn abi_replay_requires_equal_repeated_witnesses_and_continuous_budget() {
    let current = Fixture::empty();
    let value = Fixture::nominal(ConeIdentity::SINGLE_FILE, false);
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![value.exact()], value.exact());
    let single = [value.borrow()];
    let repeated = [value.borrow(), value.borrow()];
    assert_eq!(
        replay(&current, &single, &signature, &mut meter()).unwrap(),
        replay(&current, &repeated, &signature, &mut meter()).unwrap()
    );
    let changed = value.with_c_layout();
    assert!(matches!(
        replay(
            &current,
            &[value.borrow(), changed.borrow()],
            &signature,
            &mut meter()
        ),
        Err(NativeBoundaryCompileError::ConflictingTypeWitness { .. })
    ));
    let mut measured = meter();
    replay(&current, &single, &signature, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&current, &single, &signature, &mut shared).unwrap();
    assert!(matches!(
        replay(&current, &single, &signature, &mut shared),
        Err(NativeBoundaryCompileError::Resource(_))
    ));
}

fn replay(
    current: &Fixture,
    dependencies: &[AbiReplayDependency<'_>],
    signature: &ExactCallableSignature,
    meter: &mut BudgetMeter,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    replay_canonical_scoop_abi_parts(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        meter,
        &current.identities,
        &current.foundation,
        dependencies,
        signature,
        GcEffect::Managed,
    )
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
