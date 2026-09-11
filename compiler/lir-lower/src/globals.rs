use super::*;

#[derive(Clone, Copy)]
pub(super) enum StorageGlobal {
    Local(lir::GlobalId),
    Native(lir::NativeGlobalId),
}

pub(super) fn lower_globals(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    globals: &mut Arena<lir::Global>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
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
            mir::GlobalStorage::Managed { initial_state } => {
                let lir_id = globals.alloc(lir::Global {
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(context, &lir_type(&global.ty), structs, enums, 0),
                    init: lir::GlobalInit::Storage {
                        identity: static_storage_identity(
                            context,
                            identity_roots,
                            module,
                            enums,
                            global,
                        ),
                        ty: lir_type(&global.ty),
                        initial_state: lower_static_initial_state(
                            initial_state,
                            enums,
                            string_globals,
                        ),
                        thread_local: false,
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Local {
                thread_local,
                initial_state,
            } => {
                let lir_id = globals.alloc(lir::Global {
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(context, &lir_type(&global.ty), structs, enums, 0),
                    init: lir::GlobalInit::Storage {
                        identity: static_storage_identity(
                            context,
                            identity_roots,
                            module,
                            enums,
                            global,
                        ),
                        ty: lir_type(&global.ty),
                        initial_state: lower_static_initial_state(
                            initial_state,
                            enums,
                            string_globals,
                        ),
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
                let c_type = c_ffi_type(module, structs, enums, &global.ty);
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
                    c_type,
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

fn static_storage_identity(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    global: &mir::Global,
) -> lir::StaticStorageIdentity {
    let enum_shape = |id: mir::EnumId| repr_shape(context, &enums[enum_def_id(id)].repr);
    let zero_sized = size_align(context, module, &enum_shape, &global.ty).0 == 0;
    let root = identity_roots.for_static_storage(global.storage_owner);
    let identity = match global.storage_owner {
        mir::StaticStorageOwner::PropertyBacking(owner) if zero_sized => {
            lir::StaticStorageIdentity::static_place_for_property(owner, root)
        }
        mir::StaticStorageOwner::PropertyDelegate(owner) if zero_sized => {
            lir::StaticStorageIdentity::static_place_for_property(owner, root)
        }
        mir::StaticStorageOwner::PropertyBacking(owner) => {
            lir::StaticStorageIdentity::property_backing(owner, root)
        }
        mir::StaticStorageOwner::PropertyDelegate(owner) => {
            lir::StaticStorageIdentity::property_delegate(owner, root)
        }
        mir::StaticStorageOwner::SingletonPublishedRoot(owner) => {
            lir::StaticStorageIdentity::singleton_published_root(owner, root)
        }
        mir::StaticStorageOwner::InitializationFailureRoot(unit) => {
            lir::StaticStorageIdentity::initialization_failure_root(unit, root)
        }
    };
    identity.expect("validated static storage owner must derive a persistent identity")
}

pub(super) fn lower_static_initial_state(
    state: &mir::MirStaticInitialState,
    enums: &lir::EnumDefs,
    string_globals: &HashMap<mir::StringConstId, lir::GlobalId>,
) -> lir::LirStaticInitialState {
    match state {
        mir::MirStaticInitialState::ZeroedForRuntimeUnit => {
            lir::LirStaticInitialState::ZeroedForRuntimeUnit
        }
        mir::MirStaticInitialState::EncodedStaticValue { payload } => {
            lir::LirStaticInitialState::EncodedStaticValue {
                payload: lower_constant_image(payload, enums, string_globals),
            }
        }
    }
}

pub(super) fn lower_constant_image(
    value: &mir::MirConstantImage,
    enums: &lir::EnumDefs,
    string_globals: &HashMap<mir::StringConstId, lir::GlobalId>,
) -> lir::LirConstantImage {
    match value {
        mir::MirConstantImage::Integer(value) => {
            lir::LirConstantImage::Integer(integer_constant(*value))
        }
        mir::MirConstantImage::Boolean(value) => lir::LirConstantImage::Bool(*value),
        mir::MirConstantImage::String(string) => lir::LirConstantImage::GlobalPointer {
            global: string_globals[string],
            kind: lir::PointerKind::Managed,
        },
        mir::MirConstantImage::PointerNull(kind) => {
            lir::LirConstantImage::NullPointer(match kind {
                mir::MirPointerNull::Data => lir::PointerKind::Raw,
                mir::MirPointerNull::Code => lir::PointerKind::Code,
            })
        }
        mir::MirConstantImage::EnumUnit { variant } => lir::LirConstantImage::EnumUnit {
            variant: variant_ref(enums, *variant),
        },
        mir::MirConstantImage::Struct { struct_id, fields } => lir::LirConstantImage::Struct {
            struct_id: struct_def_id(*struct_id),
            fields: fields
                .iter()
                .map(|field| lower_constant_image(field, enums, string_globals))
                .collect(),
        },
    }
}
