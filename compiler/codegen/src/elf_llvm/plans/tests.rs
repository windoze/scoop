use inkwell::context::Context;
use llvm_sys::core::LLVMAddAlias2;

use super::*;

mod declarations;

#[test]
fn one_index_preserves_alias_owners_and_declaration_visibility_in_multiple_modules() {
    let index = SymbolPlanIndex {
        groups: vec!["group".into()],
        symbols: [
            (b"alias".to_vec(), LinkageClass::OdrWeak),
            (b"used".to_vec(), LinkageClass::ConeStrong),
            (b"unused".to_vec(), LinkageClass::ConeStrong),
        ]
        .into_iter()
        .enumerate()
        .map(|(order, (name, linkage))| {
            (
                name,
                vec![SymbolPlan {
                    order,
                    group: 0,
                    linkage,
                    dispatch: false,
                }],
            )
        })
        .collect(),
    };
    let context = Context::create();
    for name in ["first", "second"] {
        let llvm = context.create_module(name);
        let owner = llvm.add_global(context.i8_type(), None, "owner");
        owner.set_initializer(&context.i8_type().const_zero());
        owner.set_linkage(Linkage::Private);
        let alias = unsafe {
            GlobalValue::new(LLVMAddAlias2(
                llvm.as_mut_ptr(),
                llvm_sys::core::LLVMGlobalGetValueType(owner.as_value_ref()),
                0,
                owner.as_value_ref(),
                c"alias".as_ptr(),
            ))
        };
        let used = llvm.add_global(context.i8_type(), None, "used");
        let reference = llvm.add_global(used.as_pointer_value().get_type(), None, "reference");
        reference.set_initializer(&used.as_pointer_value());
        let unused = llvm.add_global(context.i8_type(), None, "unused");
        unused.set_visibility(GlobalVisibility::Hidden);

        index.prepare(&llvm).unwrap();
        assert_eq!(alias.get_linkage(), Linkage::WeakODR);
        assert_eq!(alias.get_visibility(), GlobalVisibility::Hidden);
        assert_eq!(owner.get_linkage(), Linkage::Private);
        assert_eq!(owner.get_visibility(), GlobalVisibility::Default);
        assert!(owner.get_comdat().is_some());
        assert_eq!(used.get_visibility(), GlobalVisibility::Hidden);
        assert_eq!(unused.get_visibility(), GlobalVisibility::Default);
        assert_eq!(reference.get_visibility(), GlobalVisibility::Default);
        llvm.verify().unwrap();
    }
}

#[test]
fn repeated_symbol_requests_still_reject_conflicting_definition_groups() {
    let context = Context::create();
    let llvm = context.create_module("conflict");
    let value = llvm.add_global(context.i8_type(), None, "shared");
    value.set_initializer(&context.i8_type().const_zero());
    let index = SymbolPlanIndex {
        groups: vec!["first".into(), "second".into()],
        symbols: HashMap::from([(
            b"shared".to_vec(),
            (0..2)
                .map(|group| SymbolPlan {
                    order: group,
                    group,
                    linkage: LinkageClass::OdrWeak,
                    dispatch: false,
                })
                .collect(),
        )]),
    };
    assert_eq!(
        index.prepare(&llvm).unwrap_err().0,
        "ELF global belongs to two definition groups: first and second"
    );
}

#[test]
fn parent_table_and_boundary_aliases_use_the_complete_arrays_comdat() {
    use inkwell::types::AsTypeRef;
    use llvm_sys::core::LLVMConstGEP2;

    let context = Context::create();
    let llvm = context.create_module("parent-view");
    let value = context.i64_type().const_array(&[
        context.i64_type().const_zero(),
        context.i64_type().const_int(1, false),
    ]);
    let owner = llvm.add_global(value.get_type(), None, "storage");
    owner.set_initializer(&value);
    let mut indices = [context.i64_type().const_int(1, false).as_value_ref()];
    let parent = unsafe {
        let pointer = LLVMConstGEP2(
            context.i64_type().as_type_ref(),
            owner.as_value_ref(),
            indices.as_mut_ptr(),
            1,
        );
        LLVMAddAlias2(
            llvm.as_mut_ptr(),
            context.i64_type().as_type_ref(),
            0,
            pointer,
            c"parent".as_ptr(),
        )
    };
    unsafe {
        LLVMAddAlias2(
            llvm.as_mut_ptr(),
            context.i64_type().as_type_ref(),
            0,
            parent,
            c"boundary".as_ptr(),
        );
    }
    let index = SymbolPlanIndex {
        groups: vec![
            "array_group".into(),
            "parent_group".into(),
            "boundary_group".into(),
        ],
        symbols: ["storage", "parent", "boundary"]
            .into_iter()
            .enumerate()
            .map(|(group, name)| {
                (
                    name.as_bytes().to_vec(),
                    vec![SymbolPlan {
                        order: 2 - group,
                        group,
                        linkage: LinkageClass::OdrWeak,
                        dispatch: true,
                    }],
                )
            })
            .collect(),
    };
    index.prepare(&llvm).unwrap();
    llvm.verify().unwrap();
    let text = llvm.print_to_string().to_string();
    assert!(text.contains("comdat($array_group)"), "{text}");
    assert!(!text.contains("$parent_group = comdat"), "{text}");
    assert!(!text.contains("$boundary_group = comdat"), "{text}");
}
