use std::collections::BTreeMap;

use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module;
use inkwell::targets::TargetMachine;
use scoop_lir::{CallerRootSource, GcEffect};

use super::{
    ExpectedRoot, ExpectedSafepoints, ExpectedSite, ExpectedStatepoint,
    TYPED_MANAGED_POINTER_BOUNDARY_METADATA, verify_rewritten as verify_rewritten_with_profile,
};
use crate::target::ManagedAddressSpace;
use crate::{CodegenError, TargetProfile};

fn verify_rewritten(
    module: &Module<'_>,
    expected: &ExpectedSafepoints,
) -> Result<(), CodegenError> {
    let triple = TargetMachine::get_default_triple();
    let profile = TargetProfile::resolve(triple.as_str().to_str().expect("UTF-8 host triple"))
        .expect("supported host target");
    verify_rewritten_with_profile(module, expected, profile.managed_address_space_contract())
}

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
    ExpectedStatepoint::Relocating(vec![ExpectedRoot {
        source: CallerRootSource::Param(0),
        byte_offset: 0,
    }])
}

fn statepoint_ir(id: u64, before: &str, after: &str) -> String {
    format!(
        r#"
declare void @callee()
declare void @consume(ptr addrspace(1))
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token, i32 immarg, i32 immarg)

define void @f(ptr addrspace(1) %root) #0 gc "statepoint-example" {{
entry:
  %slot = alloca ptr addrspace(1), align 8, !scoop.statepoint-root-identity !0
  store ptr addrspace(1) %root, ptr %slot
  %live = load volatile ptr addrspace(1), ptr %slot
  {before}
  %token = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 {id}, i32 0, ptr elementtype(void ()) @callee, i32 0, i32 0, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %live) ]
  %relocated = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %token, i32 0, i32 0)
  {after}
  ret void
}}

attributes #0 = {{ "disable-tail-calls"="true" "frame-pointer"="all" }}
!0 = !{{i64 {id}, i64 0, i64 0, i64 0}}
"#
    )
}

#[test]
fn verifier_rejects_an_unknown_safepoint_id() {
    let context = Context::create();
    let module = parse(&context, &statepoint_ir(7, "", ""));
    let error = verify_rewritten(&module, &manifest(Some((8, one_root()))))
        .expect_err("unknown id must not be accepted as a complete manifest");
    assert!(error.0.contains("unexpected SafepointId 7"), "{error}");
}

#[test]
fn verifier_consumes_the_profile_managed_address_space() {
    let context = Context::create();
    let module = parse(&context, &statepoint_ir(7, "", ""));
    let error = verify_rewritten_with_profile(
        &module,
        &manifest(Some((7, one_root()))),
        ManagedAddressSpace::for_test(7),
    )
    .expect_err("verifier must reject a root outside the profile address space");
    assert!(error.0.contains("not an AS7 managed pointer"), "{error}");
}

#[test]
fn verifier_rejects_a_safepoint_emitted_in_the_wrong_function() {
    let context = Context::create();
    let mut ir = statepoint_ir(7, "", "").replace("define void @f(", "define void @g(");
    ir.push_str(
        r#"
define void @f() #0 gc "statepoint-example" {
entry:
  ret void
}
"#,
    );
    let module = parse(&context, &ir);
    let mut expected = manifest(Some((7, one_root())));
    expected
        .functions
        .insert("g".to_string(), GcEffect::Managed);
    let error = verify_rewritten(&module, &expected)
        .expect_err("a global SafepointId cannot migrate between functions");
    assert!(error.0.contains("belongs to LIR function `f`"), "{error}");
    assert!(error.0.contains("emitted in `g`"), "{error}");
}

#[test]
fn verifier_rejects_a_gc_live_count_mismatch() {
    let context = Context::create();
    let module = parse(&context, &statepoint_ir(7, "", ""));
    let expected = ExpectedStatepoint::Relocating(vec![
        ExpectedRoot {
            source: CallerRootSource::Param(0),
            byte_offset: 0,
        },
        ExpectedRoot {
            source: CallerRootSource::Param(0),
            byte_offset: 8,
        },
    ]);
    let error = verify_rewritten(&module, &manifest(Some((7, expected))))
        .expect_err("root count must come from the complete LIR manifest");
    assert!(error.0.contains("expected 2 from"), "{error}");
    assert!(error.0.contains("observed 1"), "{error}");
}

#[test]
fn verifier_rejects_the_wrong_native_transition_entry() {
    let context = Context::create();
    let ir = r#"
declare void @scoop_rt_enter_native_safe(ptr, i64)
declare void @scoop_rt_enter_native_borrowed(ptr, i64)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)

define void @f() #0 gc "statepoint-example" {
entry:
  %token = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 7, i32 0, ptr elementtype(void (ptr, i64)) @scoop_rt_enter_native_borrowed, i32 2, i32 0, ptr null, i64 0, i32 0, i32 0)
  ret void
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
"#;
    let module = parse(&context, ir);
    let expected = ExpectedStatepoint::NativeTransition("scoop_rt_enter_native_safe");
    let error = verify_rewritten(&module, &manifest(Some((7, expected))))
        .expect_err("native safepoint identity must select the typed transition entry");
    assert!(
        error.0.contains("targets `scoop_rt_enter_native_borrowed`")
            && error.0.contains("expected `scoop_rt_enter_native_safe`"),
        "{error}"
    );
}

#[test]
fn verifier_rejects_a_gc_live_root_without_typed_identity() {
    let context = Context::create();
    let ir = statepoint_ir(7, "", "").replace(", !scoop.statepoint-root-identity !0", "");
    let module = parse(&context, &ir);
    let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect_err("every relocating root must retain its complete LIR identity");
    assert!(
        error.0.contains("lacks typed LIR identity metadata"),
        "{error}"
    );
}

#[test]
fn verifier_rejects_a_gc_live_root_with_the_wrong_typed_identity() {
    let context = Context::create();
    let ir = statepoint_ir(7, "", "").replace(
        "!0 = !{i64 7, i64 0, i64 0, i64 0}",
        "!0 = !{i64 7, i64 0, i64 1, i64 0}",
    );
    let module = parse(&context, &ir);
    let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect_err("LLVM roots may not be matched to LIR by count alone");
    assert!(
        error
            .0
            .contains("gc-live identities that disagree with complete LIR"),
        "{error}"
    );
}

#[test]
fn verifier_rejects_a_stale_root_use_after_relocation() {
    let context = Context::create();
    let module = parse(
        &context,
        &statepoint_ir(7, "", "call void @consume(ptr addrspace(1) %live)"),
    );
    let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect_err("the original root may not be consumed after a statepoint");
    assert!(error.0.contains("stale post-site use"), "{error}");
}

#[test]
fn verifier_rejects_a_derived_pointer_across_a_statepoint() {
    let context = Context::create();
    let module = parse(
        &context,
        &statepoint_ir(
            7,
            "%derived = getelementptr i8, ptr addrspace(1) %live, i64 8",
            "%byte = load i8, ptr addrspace(1) %derived",
        ),
    );
    let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect_err("an interior address must be recomputed from the relocated base");
    assert!(error.0.contains("derived AS1 address crosses"), "{error}");
}

#[test]
fn verifier_accepts_a_derived_pointer_recreated_after_each_loop_poll() {
    let context = Context::create();
    let ir = r#"
declare void @callee()
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token, i32 immarg, i32 immarg)

define void @f(ptr addrspace(1) %root, i1 %again) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8, !scoop.statepoint-root-identity !0
  store ptr addrspace(1) %root, ptr %slot
  br label %loop
loop:
  %current = load volatile ptr addrspace(1), ptr %slot
  %token = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 7, i32 0, ptr elementtype(void ()) @callee, i32 0, i32 0, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %current) ]
  %relocated = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %token, i32 0, i32 0)
  store volatile ptr addrspace(1) %relocated, ptr %slot
  %derived = getelementptr i8, ptr addrspace(1) %relocated, i64 8
  %byte = load i8, ptr addrspace(1) %derived
  br i1 %again, label %loop, label %exit
exit:
  ret void
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#;
    let module = parse(&context, ir);
    verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect("a loop iteration recomputes its derived address after relocation");
}

#[test]
fn verifier_rejects_unwitnessed_managed_pointer_integer_conversion() {
    let context = Context::create();
    let ir = r#"
define i64 @f(ptr addrspace(1) %root) #0 gc "statepoint-example" {
entry:
  %raw = ptrtoint ptr addrspace(1) %root to i64
  ret i64 %raw
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
"#;
    let module = parse(&context, ir);
    let error = verify_rewritten(&module, &manifest(None))
        .expect_err("generic managed ptrtoint must not bypass a typed boundary");
    assert!(
        error.0.contains("requires typed `card-address` boundary"),
        "{error}"
    );
}

#[test]
fn verifier_requires_the_complete_managed_function_policy() {
    let context = Context::create();
    let ir = r#"
define void @f() gc "statepoint-example" {
entry:
  ret void
}
"#;
    let module = parse(&context, ir);
    let error = verify_rewritten(&module, &manifest(None))
        .expect_err("managed frame policy is not optional");
    assert!(
        error.0.contains("lacks required `frame-pointer`"),
        "{error}"
    );
}

#[test]
fn verifier_accepts_a_witnessed_internal_pointer_boundary() {
    let context = Context::create();
    let ir = format!(
        r#"
define i64 @f(ptr addrspace(1) %root) #0 gc "statepoint-example" {{
entry:
  %raw = ptrtoint ptr addrspace(1) %root to i64, !{TYPED_MANAGED_POINTER_BOUNDARY_METADATA} !0
  ret i64 %raw
}}
attributes #0 = {{ "disable-tail-calls"="true" "frame-pointer"="all" }}
!0 = !{{!"card-address"}}
"#
    );
    let module = parse(&context, &ir);
    verify_rewritten(&module, &manifest(None))
        .expect("a compiler-emitted typed boundary witness is accepted");
}
