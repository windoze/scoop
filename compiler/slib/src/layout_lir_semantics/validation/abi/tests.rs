use super::*;
use scoop_identity::*;
use scoop_lir::LirTargetProfile;
use scoop_wire::DecodeLimits;

mod support;
use support::*;

#[test]
fn layout_abi_replay_uses_exact_types_from_local_and_dependency_providers() {
    let local = table(ConeIdentity::SINGLE_FILE, RepresentationRole::ManagedValue);
    let dependency = table(ConeIdentity::CORE, RepresentationRole::ManagedValue);
    let local_exact = local.records()[0].identity().exact();
    let external_exact = dependency.records()[0].identity().exact();
    assert_ne!(local_exact, external_exact);
    let expectation = expectation(local_exact, external_exact);
    let tables = [&local, &dependency];
    let actual = replay(&expectation, TARGET, tables.into_iter(), &mut meter()).unwrap();
    expectation.check_canonical(&actual).unwrap();
    assert_eq!(
        actual,
        replay(&expectation, TARGET, tables.into_iter().rev(), &mut meter()).unwrap()
    );
    assert!(matches!(
        replay(
            &expectation,
            TARGET,
            std::iter::once(&local),
            &mut meter()
        ),
        Err(ExactCallableAbiError::MissingValueLayout { exact }) if exact == external_exact
    ));
}

#[test]
fn layout_abi_query_rejects_duplicate_and_wrong_role_with_shared_budget() {
    let table = table(ConeIdentity::CORE, RepresentationRole::ManagedValue);
    let c_value = support::table(ConeIdentity::CORE, RepresentationRole::CValue);
    let exact = table.records()[0].identity().exact();
    assert!(matches!(
        find([&table, &table].into_iter(), exact, &mut meter()),
        Err(ExactCallableAbiError::DuplicateValueLayout { exact: found }) if found == exact
    ));
    assert!(matches!(
        find(std::iter::once(&c_value), exact, &mut meter()),
        Err(ExactCallableAbiError::MissingValueLayout { exact: found }) if found == exact
    ));
    let expectation = expectation(exact, exact);
    let run =
        |meter: &mut BudgetMeter| replay(&expectation, TARGET, std::iter::once(&table), meter);
    let mut measured = meter();
    run(&mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    run(&mut shared).unwrap();
    assert!(matches!(
        run(&mut shared),
        Err(ExactCallableAbiError::Resource(_))
    ));
}

fn expectation(local: PersistentExactTypeId, external: PersistentExactTypeId) -> AbiExpectation {
    let declaration =
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site(ConeIdentity::SINGLE_FILE),
            CanonicalIdentifier::new("mixed").unwrap(),
            0,
            None,
            vec![],
        ))
        .unwrap();
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(local),
        vec![external, local],
        external,
    );
    let storage = |exact| {
        CanonicalScoopStorage::new(
            exact,
            8,
            std::num::NonZeroU64::new(8).unwrap(),
            ScoopAbiValueShape::Scalar,
        )
    };
    AbiExpectation {
        artifact: ConeIdentity::SINGLE_FILE,
        declaration: DependencyCallableDeclarationId::Function(declaration),
        signature: signature.clone(),
        gc_effect: GcEffect::NoGc,
        actual: CanonicalScoopAbiFunctionSignature::new(
            signature,
            [local, external, local]
                .into_iter()
                .map(|exact| ScoopAbiArgument::direct(storage(exact)).unwrap())
                .collect(),
            ScoopAbiReturn::direct(storage(external)).unwrap(),
            GcEffect::NoGc,
        )
        .unwrap(),
    }
}
