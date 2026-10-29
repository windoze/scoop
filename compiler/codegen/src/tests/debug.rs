use super::*;
use object::{Object, ObjectSection};
use scoop_source::{DebugFile, DebugRecords};
use std::sync::Arc;

mod globals;
mod managed;
mod variables;

#[test]
fn dwarf_lines_and_inline_calls_survive_optimization_and_statepoints() {
    let mut module = values_module();
    let mut records = DebugRecords::default();
    let file = Arc::new(DebugFile {
        identity: Arc::new(scoop_identity::SourceIdentity::single_file()),
        path: "/workspace/debug program.scoop".into(),
    });
    let definition = records.source(file.clone(), 2, 1);
    let call = records.source(file.clone(), 7, 9);
    let scope = records.scope(None, &call);
    let call = records.scoped(&call, scope);
    let call_site = records.source(file.clone(), 12, 5);
    let inline = records.inline_site("concatenate", definition.clone(), call_site);
    let function = &mut module.functions[0];
    function.debug_name = "main".into();
    function.debug_origin = definition;
    let entry = &mut function.blocks[function.entry];
    let instruction = entry.instructions.pop().unwrap();
    assert!(matches!(instruction, Instruction::Call { .. }));
    entry
        .instructions
        .push_located(instruction, records.inlined(&call, &inline));
    entry.terminator_origin = records.source(file, 13, 1);

    let directory = tempfile::tempdir().unwrap();
    for mode in [
        scoop_lir::OptimizationMode::Debug,
        scoop_lir::OptimizationMode::Release,
    ] {
        for level in [
            scoop_lir::DebugInfoLevel::None,
            scoop_lir::DebugInfoLevel::LineTables,
            scoop_lir::DebugInfoLevel::Full,
        ] {
            let profile = host_profile()
                .with_optimization(mode)
                .with_debug_info(level);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
            llvm.verify().unwrap();
            let expected = statepoint::expectations(&module).unwrap();
            statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            llvm.verify().unwrap();
            let ir = llvm.print_to_string().to_string();
            let enabled = level != scoop_lir::DebugInfoLevel::None;
            assert_eq!(ir.contains("!DICompileUnit("), enabled, "{ir}");
            if enabled {
                for subprogram in ir.lines().filter(|line| line.contains("!DISubprogram(")) {
                    assert!(
                        !subprogram.contains("DIFlagArtificial"),
                        "source functions and inline calls remain visible: {subprogram}"
                    );
                }
                for expected in [
                    "filename: \"debug program.scoop\", directory: \"/workspace\"",
                    "name: \"main\"",
                    "name: \"concatenate\"",
                    "!DILocation(line: 7, column: 9,",
                    "!DILocation(line: 12, column: 5,",
                    "inlinedAt:",
                ] {
                    assert!(ir.contains(expected), "missing {expected}: {ir}");
                }
                assert!(ir.contains(if level == scoop_lir::DebugInfoLevel::Full {
                    "emissionKind: FullDebug"
                } else {
                    "emissionKind: LineTablesOnly"
                }));
                assert!(ir.contains(&format!(
                    "linkageName: \"{}\"",
                    module.functions[0].symbol()
                )));
            }
            let output = directory
                .path()
                .join(format!("{}-{level}.o", mode.as_str()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            let bytes = std::fs::read(output).unwrap();
            let object = object::File::parse(bytes.as_slice()).unwrap();
            let has_lines = object.sections().any(|section| {
                matches!(section.name(), Ok("__debug_line" | ".debug_line")) && section.size() > 0
            });
            assert_eq!(has_lines, enabled);
        }
    }
}

#[test]
fn generated_helpers_have_artificial_debug_subprograms() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    function.debug_origin = scoop_source::DebugOrigin::Generated;
    function.blocks[function.entry].terminator_origin = variables::origin(4);
    let profile = host_profile().with_debug_info(scoop_lir::DebugInfoLevel::Full);
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    let subprogram = ir
        .lines()
        .find(|line| line.contains("!DISubprogram("))
        .unwrap();
    assert!(subprogram.contains("DIFlagArtificial"), "{subprogram}");
}
