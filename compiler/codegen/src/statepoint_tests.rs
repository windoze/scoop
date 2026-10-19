use std::collections::BTreeMap;

use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module;
use scoop_lir::{CallerRootSource, GcEffect};

use super::{ExpectedRoot, ExpectedSafepoints, ExpectedSite, ExpectedStatepoint};
use crate::ValidatedBackendProfile;

#[path = "statepoint_tests/atomic_refs.rs"]
mod atomic_refs;
#[path = "statepoint_tests/branch_loads.rs"]
mod branch_loads;
#[path = "statepoint_tests/call_frame.rs"]
mod call_frame;
#[path = "statepoint_tests/constant_roots.rs"]
mod constant_roots;
#[path = "statepoint_tests/copied_aggregate.rs"]
mod copied_aggregate;
#[path = "statepoint_tests/guarded_unwrap.rs"]
mod guarded_unwrap;
#[path = "statepoint_tests/interface_parts.rs"]
mod interface_parts;
#[path = "statepoint_tests/loops.rs"]
mod loops;
#[path = "statepoint_tests/nonduplicable.rs"]
mod nonduplicable;
#[path = "statepoint_tests/optimized.rs"]
mod optimized;
#[path = "statepoint_tests/terminal_poll.rs"]
mod terminal_poll;

#[path = "statepoint_tests/tail_duplication.rs"]
mod tail_duplication;

fn parse<'ctx>(context: &'ctx Context, ir: &str) -> Module<'ctx> {
    let buffer = MemoryBuffer::create_from_memory_range_copy(ir.as_bytes(), "statepoint-test");
    let module = context
        .create_module_from_ir(buffer)
        .expect("test LLVM IR parses");
    module.verify().expect("test LLVM IR verifies");
    module
}

fn manifest(site: Option<(u64, ExpectedStatepoint)>) -> ExpectedSafepoints {
    ExpectedSafepoints {
        sites: site
            .map(|(id, statepoint)| {
                (
                    id,
                    ExpectedSite {
                        function: "f".to_string(),
                        block: "entry".to_string(),
                        statepoint,
                    },
                )
            })
            .into_iter()
            .collect(),
        functions: BTreeMap::from([("f".to_string(), GcEffect::Managed)]),
    }
}

fn one_root() -> ExpectedStatepoint {
    ExpectedStatepoint::Relocating(
        vec![ExpectedRoot {
            source: CallerRootSource::Param(0),
            byte_offset: 0,
        }]
        .into(),
    )
}

fn assert_machine_site_count(path: &std::path::Path, expected: usize) {
    use object::{Object, ObjectSection};
    let bytes = std::fs::read(path).unwrap();
    let object = object::File::parse(bytes.as_slice()).unwrap();
    let section = object
        .section_by_name("__llvm_stackmaps")
        .or_else(|| object.section_by_name(".llvm_stackmaps"))
        .unwrap();
    let data = section.data().unwrap();
    assert_eq!(data[0], 3);
    assert_eq!(
        u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize,
        expected
    );
}

fn assert_machine_root_counts(path: &std::path::Path, expected: &ExpectedSafepoints) {
    use object::{Object, ObjectSection};
    let bytes = std::fs::read(path).unwrap();
    let object = object::File::parse(bytes.as_slice()).unwrap();
    let section = object
        .section_by_name("__llvm_stackmaps")
        .or_else(|| object.section_by_name(".llvm_stackmaps"))
        .unwrap();
    let parsed = scoop_slib::parse_llvm_stackmap_section_v3(section.data().unwrap()).unwrap();
    let actual = parsed
        .functions()
        .iter()
        .flat_map(|function| function.records())
        .map(|record| (record.safepoint_id(), record.locations().len()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(parsed.record_count(), actual.len());
    let expected = expected
        .sites
        .keys()
        .map(|id| (*id, 3 + 2 * expected.root_count(*id).unwrap()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}
