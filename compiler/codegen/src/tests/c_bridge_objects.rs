use super::*;

#[test]
fn empty_bridge_plan_produces_an_empty_owned_object_set_without_compiling() {
    let module = values_module();
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
    let parent = tempfile::tempdir().unwrap();

    let emitted = crate::c_bridge_emission::emit_c_bridge_object_set_with_compiler_for_test(
        sources,
        parent.path(),
        generated_c_profile(),
        |_source, _object| -> Result<(), CodegenError> {
            panic!("an empty bridge plan must not invoke the compiler")
        },
    )
    .unwrap();

    assert!(emitted.sources().plan().units().is_empty());
    assert!(emitted.members().is_empty());
    assert!(emitted.temporary_directory().starts_with(parent.path()));
}

#[test]
fn generated_c_object_set_preserves_unit_authority_and_immutable_backing() {
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
        |source, object| {
            assert!(
                std::fs::metadata(source)
                    .map_err(|error| CodegenError(error.to_string()))?
                    .permissions()
                    .readonly()
            );
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
        assert!(
            std::fs::metadata(member.source_path())
                .unwrap()
                .permissions()
                .readonly()
        );
        let object = std::fs::metadata(member.object_path()).unwrap();
        assert!(object.is_file() && object.len() > 0 && object.permissions().readonly());
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
        |_source, object| {
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

#[test]
fn generated_c_object_set_rejects_a_missing_compiler_output() {
    let module = bridge_module(1);
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
    let parent = tempfile::tempdir().unwrap();

    let error = crate::c_bridge_emission::emit_c_bridge_object_set_with_compiler_for_test(
        sources,
        parent.path(),
        generated_c_profile(),
        |_source, _object| Ok(()),
    )
    .unwrap_err();

    assert!(
        error.0.contains("cannot inspect generated-C object"),
        "{error}"
    );
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

fn bridge_module(count: u8) -> Module {
    let mut module = values_module();
    for seed in 1..=count {
        install_test_native_function_contract(&mut module, seed);
        module.extern_functions.alloc_c(scoop_lir::CExternFunction {
            identity: scoop_lir::ExternFunctionIdentity {
                source_name: format!("bridge{seed}"),
                native_symbol: format!("native_bridge_{seed}"),
                library: "fixture".to_owned(),
                calling_convention: scoop_lir::CallingConvention::Cdecl,
            },
            bridge: outbound_bridge(seed),
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
    scoop_lir::CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(deployment, compiler)
        .unwrap()
}
