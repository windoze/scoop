use super::*;

mod imported;

#[derive(Clone, Copy)]
pub(super) enum StorageGlobal {
    Local(lir::GlobalId),
    Native(lir::NativeGlobalId),
}

pub(super) struct GlobalLoweringInputs<'a> {
    pub(super) context: &'a LoweringContext,
    pub(super) identity_roots: &'a IdentityRoots<'a>,
    pub(super) module: &'a mir::Module,
    pub(super) selected_layout: Option<&'a lir::StrongProductionDependencySelectionV2<'a>>,
    pub(super) structs: &'a lir::StructDefs,
    pub(super) enums: &'a lir::EnumDefs,
    pub(super) string_globals: &'a HashMap<mir::StringConstId, lir::GlobalId>,
    pub(super) native_externals: &'a lir::NativeExternalMetadata,
}

pub(super) struct LoweredGlobals {
    pub(super) storage: HashMap<mir::GlobalId, StorageGlobal>,
    pub(super) native: Arena<lir::NativeGlobal>,
    pub(super) bridges: lir::NativeGlobalBridges,
}

pub(super) fn lower_globals(
    inputs: GlobalLoweringInputs<'_>,
    globals: &mut Arena<lir::Global>,
) -> Result<LoweredGlobals, LirLoweringError> {
    let GlobalLoweringInputs {
        context,
        identity_roots,
        module,
        selected_layout,
        structs,
        enums,
        string_globals,
        native_externals,
    } = inputs;
    let mut map = HashMap::new();
    let mut native = Arena::new();
    let mut bridges = lir::NativeGlobalBridges::default();
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            mir::GlobalStorage::Imported { provider, storage } => StorageGlobal::Local(
                globals.alloc(imported::lower(&inputs, global, *provider, *storage)?),
            ),
            mir::GlobalStorage::Managed { initial_state } => {
                let lir_id = globals.alloc(lir::Global {
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(
                        context,
                        &lir_type(module, &global.ty),
                        structs,
                        enums,
                        0,
                    )?,
                    init: lir::GlobalInit::Storage {
                        identity: static_storage_identity(
                            context,
                            identity_roots,
                            module,
                            enums,
                            global,
                        )?,
                        layout: static_storage_layout(
                            context,
                            identity_roots,
                            module,
                            &global.ty,
                            selected_layout,
                        )?,
                        ty: lir_type(module, &global.ty),
                        initial_state: lower_static_initial_state(
                            initial_state,
                            enums,
                            string_globals,
                        ),
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Local {
                thread_local,
                initializer,
            } => {
                let lir_id = globals.alloc(lir::Global {
                    address_kind: lir::PointerKind::Raw,
                    scan: lir::RefScan::None,
                    init: lir::GlobalInit::RawStorage {
                        identity: static_storage_identity(
                            context,
                            identity_roots,
                            module,
                            enums,
                            global,
                        )?,
                        ty: lir_type(module, &global.ty),
                        initializer: lower_constant_image(initializer, enums, string_globals),
                        thread_local: *thread_local,
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Extern {
                source_contract,
                library,
                native_symbol,
                thread_local,
            } => {
                let c_type = c_ffi_type(module, structs, enums, &global.ty);
                let contract = native_externals
                    .contract(source_contract.id())
                    .expect("every native global has one normalized target contract")
                    .fingerprint();
                let get = bridges.gets.alloc(lir::NativeGlobalGetBridge {
                    identity: lir::GeneratedBridgeEntryIdentity::new(
                        module.cone,
                        scoop_identity::GeneratedBridgeUnitKey::GlobalRead(contract),
                    )
                    .expect("validated native-global read bridge identities are encodable"),
                });
                let address = bridges.addresses.alloc(lir::NativeGlobalAddressBridge {
                    identity: lir::GeneratedBridgeEntryIdentity::new(
                        module.cone,
                        scoop_identity::GeneratedBridgeUnitKey::GlobalAddress(contract),
                    )
                    .expect("validated native-global address bridge identities are encodable"),
                });
                let access = if global.mutable {
                    let set = bridges.sets.alloc(lir::NativeGlobalSetBridge {
                        identity: lir::GeneratedBridgeEntryIdentity::new(
                            module.cone,
                            scoop_identity::GeneratedBridgeUnitKey::GlobalWrite(contract),
                        )
                        .expect("validated native-global write bridge identities are encodable"),
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
    Ok(LoweredGlobals {
        storage: map,
        native,
        bridges,
    })
}

fn static_storage_identity(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    global: &mir::Global,
) -> StorageResult<lir::StaticStorageIdentity> {
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let zero_sized = size_align(context, module, &enum_shape, &global.ty)?.0 == 0;
    let root = identity_roots.for_static_storage(global.storage_owner);
    let identity = match global.storage_owner {
        mir::StaticStorageOwner::GenericDelegate(unit) => {
            let unit = module
                .initialization_units
                .iter()
                .find_map(|(_, value)| (value.identity.id() == unit).then_some(value))
                .expect("delegate storage retains its initialization unit");
            return Ok(lir::StaticStorageIdentity::delegated_application(
                unit.identity.key(),
                zero_sized,
                root,
            )
            .expect("a generic delegate storage derives from its application unit"));
        }
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
        mir::StaticStorageOwner::SingletonApplicationPublishedRoot(owner) => {
            lir::StaticStorageIdentity::singleton_application_root(owner, root)
        }
        mir::StaticStorageOwner::SingletonPublishedRoot(owner) => {
            lir::StaticStorageIdentity::singleton_published_root(owner, root)
        }
        mir::StaticStorageOwner::InitializationFailureRoot(unit) => {
            lir::StaticStorageIdentity::initialization_failure_root(unit, root)
        }
    };
    Ok(identity.expect("validated static storage owner must derive a persistent identity"))
}

pub(super) fn static_storage_layout(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    ty: &mir::Type,
    selected: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
) -> Result<lir::StaticStorageLayout, LirLoweringError> {
    use scoop_identity::RepresentationRole;
    let exact = exact_type_record(module, ty).id();
    let target = context.target_profile();
    let role = match ty {
        mir::Type::Struct(id)
            if matches!(
                &module.structs[*id].representation,
                mir::StructRepresentation::Declared {
                    c_layout: Some(_),
                    ..
                }
            ) =>
        {
            RepresentationRole::CValue
        }
        mir::Type::FunPtr(_) => RepresentationRole::NativeFunctionPointer,
        _ => RepresentationRole::ManagedValue,
    };
    if let Some(source) = module.meta.source_exact_types.get(ty)
        && let mir::SourceExactTypeOwner::Cone(provider) = source.owner()
        && provider != module.cone
    {
        let selected = selected
            .ok_or(LirLoweringError::MissingDependencyLayoutSelection { provider, exact })?;
        let layout = scoop_identity::PersistentLayoutId::from_key(&scoop_identity::LayoutKey::new(
            exact,
            target.wire_id(),
            role,
        ))
        .expect("a typed value layout key is encodable");
        let value = match selected
            .semantic_record(provider, lir::LayoutAbiSemanticTargetV1::Layout(layout))
        {
            Some(lir::LayoutAbiSemanticRecordV1::Layout(record)) => record.value_handle(),
            _ => None,
        }
        .ok_or(LirLoweringError::MissingDependencyValueLayout { provider, layout })?;
        return Ok(lir::StaticStorageLayout::External(value));
    }
    let root = identity_roots.for_type(ty);
    let identity = match role {
        RepresentationRole::CValue => lir::LayoutIdentity::c_value(exact, target, root),
        RepresentationRole::NativeFunctionPointer => {
            lir::LayoutIdentity::native_function_pointer(exact, target, root)
        }
        RepresentationRole::ManagedValue => lir::LayoutIdentity::managed_value(exact, target, root),
        RepresentationRole::ManagedObject => {
            unreachable!("static storage always has a value representation")
        }
    }
    .expect("a typed storage value derives a layout identity");
    Ok(identity.into())
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
        mir::MirConstantImage::Float(value) => lir::LirConstantImage::Float(*value),
        mir::MirConstantImage::Char(value) => {
            lir::LirConstantImage::Integer(lir::LirIntegerConstant::Signed32(*value as u32))
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
