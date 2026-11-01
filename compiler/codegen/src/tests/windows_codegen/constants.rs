use scoop_lir::{CallableConstantKind, FloatBinaryOperator, FloatKind, FloatUnaryOperator};

use super::*;

pub(super) fn floating_function(name: &str, odr: bool) -> Function {
    let mut temps = Arena::new();
    let product = temps.alloc(Temp { ty: LirType::F64 });
    let sum = temps.alloc(Temp { ty: LirType::F64 });
    let result = temps.alloc(Temp { ty: LirType::F64 });
    let mut blocks = Arena::new();
    let float = |value: f64| Value::FloatConst(scoop_lir::LirFloatConstant::F64(value.to_bits()));
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![
            Instruction::FloatBinary {
                out: product,
                kind: FloatKind::F64,
                operation: FloatBinaryOperator::Multiply,
                lhs: Value::Param(0),
                rhs: float(1.25),
            },
            Instruction::FloatBinary {
                out: sum,
                kind: FloatKind::F64,
                operation: FloatBinaryOperator::Add,
                lhs: Value::Temp(product),
                rhs: float(7.5),
            },
            Instruction::FloatUnary {
                out: result,
                kind: FloatKind::F64,
                operation: FloatUnaryOperator::Negate,
                operand: Value::Temp(sum),
            },
        ]
        .into(),
        terminator_origin: Default::default(),
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_signature: Default::default(),
        debug_origin: Default::default(),
        callable_body: if odr {
            odr_callable_body(name)
        } else {
            callable_body(name)
        },
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(vec![LirType::F64], LirType::F64),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals: Arena::new(),
        temps,
        blocks,
        entry,
    }
}

#[test]
fn windows_constant_pools_follow_callable_selection_and_preserve_alignment() {
    #[cfg(windows)]
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        let mut module = values_module();
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        module.output = scoop_lir::LirOutput::Library;
        module.globals = Arena::new();
        module.functions = vec![
            floating_function("coffFloatStrong", false),
            floating_function("coffFloatOdr", true),
        ];
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap()
                .with_optimization(scoop_lir::OptimizationMode::Release);
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            profile,
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error}"));
        let mut callables = Vec::new();
        for member in emitted
            .members
            .iter()
            .filter(|member| matches!(member.units.kind(), ScoopLirObjectKindV1::CallableBody(_)))
        {
            let plan = member
                .units
                .definition_plans()
                .iter()
                .filter_map(|id| emitted.production.canonical_definitions().plan(*id))
                .find(|plan| {
                    plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                })
                .unwrap();
            let bytes = member.bytes.clone();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(
                input.module().meta.target_profile,
                &bytes,
            )
            .unwrap();
            let atom =
                crate::callable_constants::atom(plan, CallableConstantKind::Relocatable).unwrap();
            let boundary = plan
                .atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom)
                .unwrap();
            let start = file
                .symbol_by_name(boundary.start().symbol().as_str())
                .unwrap();
            let end = file
                .symbol_by_name(boundary.end().symbol().as_str())
                .unwrap();
            let pool = file
                .section_by_index(start.section_index().unwrap())
                .unwrap();
            assert_eq!(pool.name().unwrap(), ".rdata");
            assert_eq!(end.address() - start.address(), pool.size());
            assert_eq!(
                file.sections()
                    .filter(|section| section.size() != 0 && section.name() == Ok(".rdata"))
                    .count(),
                1
            );
            assert!(pool.align() >= 16);
            let text = file
                .symbol_by_name(plan.primary_symbol().symbol().as_str())
                .unwrap()
                .section_index()
                .unwrap();
            assert!(file.comdats().any(|group| {
                let sections = group.sections().collect::<Vec<_>>();
                sections.contains(&text) && sections.contains(&pool.index())
            }));
            for symbol in file.symbols().filter(|symbol| {
                symbol
                    .name()
                    .is_ok_and(|name| name.starts_with("__real@") || name.starts_with("__xmm@"))
            }) {
                assert!(symbol.is_local());
            }
            callables.push((member.bytes.as_slice(), plan.primary_symbol()));
        }
        assert_eq!(callables.len(), 2);
        #[cfg(windows)]
        link_and_run(directory.path(), target, profile.llvm_triple(), &callables);
    }
}

#[cfg(windows)]
fn link_and_run(
    directory: &std::path::Path,
    target: TargetProfileId,
    triple: &str,
    callables: &[(&[u8], scoop_lir::PersistentSymbolRequest)],
) {
    let callables = callables
        .iter()
        .enumerate()
        .map(|(index, (bytes, symbol))| {
            let path = directory.join(format!("callable-{index}.o"));
            std::fs::write(&path, bytes).unwrap();
            (path, *symbol)
        })
        .collect::<Vec<_>>();
    let source = directory.join("coff-constants.c");
    std::fs::write(&source, format!(
        "extern double first(double) __asm__(\"{}\");\nextern double second(double) __asm__(\"{}\");\nint main(void) {{ return first(2.0) != -10.0 || second(-2.0) != -5.0 || first == second; }}\n",
        callables[0].1.symbol(), callables[1].1.symbol()
    )).unwrap();
    let duplicate = directory.join("coff-odr-copy.o");
    let odr = callables
        .iter()
        .find(|(_, symbol)| symbol.linkage() == scoop_lir::LinkageClass::OdrWeak)
        .unwrap();
    std::fs::copy(&odr.0, &duplicate).unwrap();
    let executable = directory.join("coff-constants.exe");
    let output = native_command(target, triple)
        .arg(&source)
        .args(callables.iter().map(|(path, _)| path))
        .arg(&duplicate)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{target:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(executable)
            .status()
            .unwrap()
            .success()
    );
}
