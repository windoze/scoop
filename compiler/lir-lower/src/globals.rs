use super::*;

#[derive(Clone, Copy)]
pub(super) enum StorageGlobal {
    Local(lir::GlobalId),
    Native(lir::NativeGlobalId),
}

pub(super) fn lower_globals(
    module: &mir::Module,
    globals: &mut Arena<lir::Global>,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    string_globals: &HashMap<mir::StringConstId, lir::GlobalId>,
) -> (
    HashMap<mir::GlobalId, StorageGlobal>,
    Arena<lir::NativeGlobal>,
    lir::NativeGlobalBridges,
) {
    let mut map = HashMap::new();
    let mut native = Arena::new();
    let mut bridges = lir::NativeGlobalBridges::default();
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            mir::GlobalStorage::Managed { initializer } => {
                let lir_id = globals.alloc(lir::Global {
                    symbol: global.symbol.clone(),
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(&lir_type(&global.ty), structs, enums, 0),
                    init: lir::GlobalInit::Storage {
                        ty: lir_type(&global.ty),
                        initializer: lower_constant(initializer, string_globals),
                        thread_local: false,
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Local {
                thread_local,
                initializer,
            } => {
                let lir_id = globals.alloc(lir::Global {
                    symbol: global.symbol.clone(),
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(&lir_type(&global.ty), structs, enums, 0),
                    init: lir::GlobalInit::Storage {
                        ty: lir_type(&global.ty),
                        initializer: lower_constant(initializer, string_globals),
                        thread_local: *thread_local,
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => {
                let raw = native.len() as u32;
                let get = bridges.gets.alloc(lir::NativeGlobalGetBridge {
                    symbol: format!("scoop_c_global_get_{raw}"),
                });
                let address = bridges.addresses.alloc(lir::NativeGlobalAddressBridge {
                    symbol: format!("scoop_c_global_address_{raw}"),
                });
                let access = if global.mutable {
                    let set = bridges.sets.alloc(lir::NativeGlobalSetBridge {
                        symbol: format!("scoop_c_global_set_{raw}"),
                    });
                    lir::NativeGlobalAccess::Mutable { get, set, address }
                } else {
                    lir::NativeGlobalAccess::ReadOnly { get, address }
                };
                let lir_id = native.alloc(lir::NativeGlobal {
                    source_name: global.name.clone(),
                    native_symbol: native_symbol.clone(),
                    library: library.clone(),
                    ty: lir_type(&global.ty),
                    c_type: c_ffi_type(module, &global.ty),
                    thread_local: *thread_local,
                    access,
                });
                StorageGlobal::Native(lir_id)
            }
        };
        map.insert(id, storage);
    }
    (map, native, bridges)
}

pub(super) fn lower_constant(
    value: &mir::ConstantValue,
    string_globals: &HashMap<mir::StringConstId, lir::GlobalId>,
) -> lir::ConstantValue {
    match value {
        mir::ConstantValue::Zero => lir::ConstantValue::Zero,
        mir::ConstantValue::Int(value) => lir::ConstantValue::Int(*value),
        mir::ConstantValue::Bool(value) => lir::ConstantValue::Bool(*value),
        mir::ConstantValue::String(string) => lir::ConstantValue::GlobalPointer {
            global: string_globals[string],
            kind: lir::PointerKind::Managed,
        },
        mir::ConstantValue::NullPtr => lir::ConstantValue::NullPointer(lir::PointerKind::Raw),
        mir::ConstantValue::NullFunPtr => lir::ConstantValue::NullPointer(lir::PointerKind::Code),
        mir::ConstantValue::EnumUnit { enum_id, variant } => lir::ConstantValue::EnumUnit {
            enum_id: enum_def_id(*enum_id),
            variant: *variant,
        },
        mir::ConstantValue::Struct { struct_id, fields } => lir::ConstantValue::Struct {
            struct_id: struct_def_id(*struct_id),
            fields: fields
                .iter()
                .map(|field| lower_constant(field, string_globals))
                .collect(),
        },
    }
}
