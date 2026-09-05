use super::*;

mod support;

use support::*;

#[test]
fn no_gc_effect_is_preserved_in_lir() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    builder.functions[main].gc_effect = mir::GcEffect::NoGc;
    let module = lower(&builder.finish(main));
    assert_eq!(module.functions[0].gc_effect, lir::GcEffect::NoGc);
    assert!(lir::dump(&module).contains("-> void <no-gc>"));
}

#[test]
fn foreign_callback_bridge_preserves_its_nominal_family() {
    let mut builder = Builder::new();
    let callback = builder.structs.alloc(mir::StructDef {
        name: "ForeignCallback<(Int) -> Unit>".to_string(),
        gc_free: true,
        representation: mir::StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![
                mir::Field {
                    name: "function".to_string(),
                    ty: mir::Type::FunPtr(mir::FunctionTypeId::from_raw(0.into())),
                },
                mir::Field {
                    name: "context".to_string(),
                    ty: mir::Type::Ptr(Box::new(mir::Type::Unit)),
                },
            ],
        },
    });
    let unit_variant = |name: &str| mir::VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: Vec::new(),
    };
    let state = builder.enums.alloc(mir::EnumDef {
        name: "ForeignCallbackState".to_string(),
        gc_free: true,
        variants: ["Registered", "Active", "Completed", "Failed"]
            .map(unit_variant)
            .into(),
    });
    let failure = builder.option_enum("Option<Any>", mir::Type::Any);
    let main = builder.main(Arena::new(), Vec::new());
    let mut module = builder.finish(main);
    let native_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Ptr(Box::new(mir::Type::Unit))],
        return_type: mir::Type::Unit,
    });
    let managed_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let family = module
        .foreign_callback_families
        .alloc(mir::ForeignCallbackFamily {
            callback,
            state,
            failure,
        });
    let adapter = module
        .foreign_callback_adapters
        .alloc(mir::ForeignCallbackAdapter {
            function: main,
            managed_signature,
        });
    module
        .foreign_callback_bridges
        .alloc(mir::ForeignCallbackBridge {
            adapter,
            family,
            native_signature,
            context_index: 0,
            mode: mir::ForeignCallbackMode::Reusable,
        });

    let lowered = lower(&module);
    let lowered_family = lowered.foreign_callback_families.iter().next().unwrap().1;
    assert_eq!(lowered_family.callback.into_raw(), callback.into_raw());
    assert_eq!(lowered_family.state.into_raw(), state.into_raw());
    assert_eq!(lowered_family.failure.into_raw(), failure.into_raw());
    assert_eq!(
        lowered
            .foreign_callback_bridges
            .iter()
            .next()
            .unwrap()
            .1
            .family,
        scoop_lir::ForeignCallbackFamilyId::from_raw(family.into_raw())
    );
}

mod arrays;
mod basics;
mod enums;
mod exceptions;
mod functions;
mod objects;
mod traps;
