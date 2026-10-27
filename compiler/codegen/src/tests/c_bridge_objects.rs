use super::*;

#[cfg(windows)]
mod windows;

#[test]
fn empty_bridge_plan_produces_an_empty_owned_object_set_without_compiling() {
    let module = values_module();
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
    let parent = tempfile::tempdir().unwrap();

    let emitted = crate::c_bridge_emission::emit_c_bridge_object_set_with_compiler_for_test(
        sources,
        parent.path(),
        generated_c_profile(),
        |_plan, _source, _object| -> Result<(), CodegenError> {
            panic!("an empty bridge plan must not invoke the compiler")
        },
    )
    .unwrap();

    assert!(emitted.sources().plan().units().is_empty());
    assert!(emitted.members().is_empty());
    assert_eq!(
        emitted.production(),
        &scoop_lir::CBridgeProductionSetV1::NotUsed
    );
    assert!(emitted.temporary_directory().starts_with(parent.path()));
}

#[test]
fn generated_c_object_set_preserves_units_and_owns_temporary_files() {
    let module = bridge_module(2);
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
    let expected_units = sources
        .plan()
        .units()
        .iter()
        .map(scoop_lir::GeneratedBridgeUnitPlanV1::unit)
        .collect::<Vec<_>>();
    let profile = generated_c_profile();
    let parent = tempfile::tempdir().unwrap();
    let mut compiled_units = Vec::new();

    let emitted = crate::c_bridge_emission::emit_c_bridge_object_set_with_compiler_for_test(
        sources,
        parent.path(),
        profile.clone(),
        |_plan, source, object| {
            let unit = source
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap()
                .to_owned();
            compiled_units.push(unit.clone());
            std::fs::write(object, format!("object:{unit}"))
                .map_err(|error| CodegenError(error.to_string()))?;
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(emitted.profile(), &profile);
    let scoop_lir::CBridgeProductionSetV1::Used(production) = emitted.production() else {
        panic!("non-empty bridge plan must retain used production authority")
    };
    assert_eq!(production.units(), expected_units);
    assert_eq!(emitted.sources().plan().units().len(), 2);
    assert_eq!(
        emitted
            .members()
            .iter()
            .map(EmittedGeneratedCBridgeObjectMemberV1::unit)
            .collect::<Vec<_>>(),
        expected_units
    );
    assert_eq!(
        compiled_units,
        expected_units
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    for member in emitted.members() {
        assert_eq!(member.unit(), member.plan().unit());
        assert_eq!(
            member.source_path().parent(),
            Some(emitted.temporary_directory())
        );
        assert_eq!(
            member.object_path().parent(),
            Some(emitted.temporary_directory())
        );
        assert!(member.source_path().is_file());
        assert!(!std::fs::read(member.object_path()).unwrap().is_empty());
    }
    let backing = emitted.temporary_directory().to_path_buf();
    drop(emitted);
    assert!(!backing.exists());
}

#[test]
fn generated_c_object_set_discards_all_partial_outputs_on_compiler_failure() {
    let module = bridge_module(2);
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let mut compilation = 0;

    let error = crate::c_bridge_emission::emit_c_bridge_object_set_with_compiler_for_test(
        sources,
        parent.path(),
        generated_c_profile(),
        |_plan, _source, object| {
            compilation += 1;
            if compilation == 1 {
                std::fs::write(object, b"first object")
                    .map_err(|error| CodegenError(error.to_string()))?;
                Ok(())
            } else {
                Err(CodegenError("test compiler failure".to_owned()))
            }
        },
    )
    .unwrap_err();

    assert!(error.0.contains("test compiler failure"), "{error}");
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

fn bridge_module(count: u8) -> Module {
    bridge_module_for_target(count, scoop_target_data::DARWIN_AARCH64)
}

fn bridge_module_for_target(count: u8, target: scoop_lir::LirTargetProfile) -> Module {
    let mut module = values_module();
    module.meta = string_metadata_for(target);
    for seed in 1..=count {
        let contract = native_function_contract_for_target(
            seed,
            c_signature(seed).signature().clone(),
            target,
        );
        let entry = outbound_bridge_for_contract(&contract);
        install_test_native_contract(&mut module, contract);
        module.extern_functions.alloc_c(scoop_lir::CExternFunction {
            call_mode: scoop_identity::CAbiCallMode::NativeSafe,
            identity: scoop_lir::ExternFunctionIdentity {
                source_name: format!("bridge{seed}"),
                native_symbol: format!("test_bridge_{seed}"),
                library: "fixture".to_owned(),
                calling_convention: scoop_lir::CallingConvention::Cdecl,
            },
            call_plan: scoop_lir::CAbiCallPlan::StorageBridge {
                entry: Box::new(entry),
                result: scoop_identity::CResultAdaptation::Direct,
            },
            signature: scoop_lir::CFunctionType {
                params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_32)],
                return_type: scoop_lir::CReturnType::Void,
            },
        });
    }
    module
}

fn generated_c_profile() -> scoop_lir::CBridgeToolchainProfileV1 {
    let deployment = scoop_lir::DarwinCBridgeDeploymentContractV1::new(
        scoop_lir::DarwinPackedVersionV1::from_components(15, 0, 0).unwrap(),
        scoop_lir::DarwinPackedVersionV1::from_components(26, 0, 0).unwrap(),
        Vec::new(),
    )
    .unwrap();
    let compiler = scoop_lir::AppleClangCompilerIdentityV1::new(21, 0, 0, "test-clang").unwrap();
    scoop_target_data::darwin_c_bridge_profile(deployment, compiler).unwrap()
}
