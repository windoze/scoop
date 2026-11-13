use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetTriple};

use super::*;

#[test]
fn indexed_reader_preserves_zero_records_and_rejects_invalid_input() {
    // LLVM 22.1 llvm-profdata merge, without sparse mode, from two text records:
    // live: hash 42, counts [7, 0, 11]; cold: hash 17, counts [0, 0].
    let bytes = include_str!("llvm22-profile.hex")
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect::<Vec<_>>();
    let mut records = decode_indexed_profile(&bytes).unwrap();
    records.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(
        records,
        vec![
            IndexedRecord {
                name: b"cold".to_vec(),
                hash: 17,
                counts: vec![0, 0]
            },
            IndexedRecord {
                name: b"live".to_vec(),
                hash: 42,
                counts: vec![7, 0, 11]
            },
        ]
    );
    for input in [
        b"".as_slice(),
        b"live\n42\n1\n7\n",
        &bytes[..8],
        &bytes[..bytes.len() / 2],
    ] {
        assert!(decode_indexed_profile(input).is_err());
    }
}

#[test]
fn llvm_counters_use_atomic_updates_and_odr_groups_on_supported_formats() {
    Target::initialize_aarch64(&InitializationConfig::default());
    Target::initialize_x86(&InitializationConfig::default());
    for target in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-gnu",
    ] {
        let triple = TargetTriple::create(target);
        let machine = Target::from_triple(&triple)
            .unwrap()
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::None,
                RelocMode::PIC,
                CodeModel::Default,
            )
            .unwrap();
        let context = Context::create();
        let module = context.create_module("profile");
        module.set_triple(&triple);
        module.set_data_layout(&machine.get_target_data().get_data_layout());
        let builder = context.create_builder();
        for (name, linkage) in [("live", Linkage::External), ("shared", Linkage::WeakODR)] {
            let function =
                module.add_function(name, context.void_type().fn_type(&[], false), Some(linkage));
            if linkage == Linkage::WeakODR && !target.contains("darwin") {
                function
                    .as_global_value()
                    .set_comdat(module.get_or_insert_comdat(name));
            }
            let entry = context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            let record = LlvmProfileRecord::new(&module, function, 42, 3).unwrap();
            record.increment(&builder, 0).unwrap();
            record.increment(&builder, 2).unwrap();
            assert!(record.increment(&builder, 3).is_err());
            builder.build_return(None).unwrap();
        }
        module.verify().unwrap();
        lower_counters(&module, &machine);
        module.verify().unwrap();
        let text = module.print_to_string().to_string();
        let updates = text
            .lines()
            .filter(|line| line.contains("atomicrmw add"))
            .collect::<Vec<_>>();
        assert_eq!(updates.len(), 4, "{target}: {text}");
        assert!(updates.iter().all(|line| line.contains("i64 1 monotonic")));
        assert!(!text.contains("call void @llvm.instrprof.increment"));
        let counters = module.get_global("__profc_shared").unwrap();
        assert_eq!(counters.get_value_type().into_array_type().len(), 3);
        if !target.contains("darwin") {
            assert!(counters.get_comdat().is_some());
        }
    }
}
