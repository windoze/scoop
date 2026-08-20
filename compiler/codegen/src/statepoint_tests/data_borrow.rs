use super::*;

#[test]
fn scoped_data_borrow_allows_only_the_typed_managed_to_raw_conversion() {
    let context = Context::create();
    for (source, target, annotation, accepted) in [
        ("ptr addrspace(1)", "ptr", "scoped-data-borrow", true),
        ("ptr addrspace(1)", "ptr", "card-address", false),
        ("ptr", "ptr addrspace(1)", "scoped-data-borrow", false),
        (
            "ptr addrspace(1)",
            "ptr addrspace(2)",
            "scoped-data-borrow",
            false,
        ),
    ] {
        let ir = format!(
            r#"
define {target} @f({source} %object) #0 gc "statepoint-example" {{
entry:
  %data = addrspacecast {source} %object to {target}, !{TYPED_MANAGED_POINTER_BOUNDARY_METADATA} !0
  ret {target} %data
}}
attributes #0 = {{ "disable-tail-calls"="true" "frame-pointer"="all" }}
!0 = !{{!"{annotation}"}}
"#
        );
        let module = parse(&context, &ir);
        assert_eq!(
            verify_rewritten(&module, &manifest(None)).is_ok(),
            accepted,
            "{source} -> {target} tagged {annotation}"
        );
    }
}
