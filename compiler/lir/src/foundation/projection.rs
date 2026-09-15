use super::*;
use crate::{Function, Module};

impl CanonicalLirFoundation {
    /// Project the identities introduced by final LIR entities into the
    /// arena-independent foundation tables.
    ///
    /// The projection reads the complete identity relations owned by each
    /// function. It never scans instructions to reconstruct callable-body or
    /// safepoint semantics from local arena ids.
    pub fn from_module(module: &Module) -> Result<Self, LirFoundationBuildError> {
        let mut foundation = Self::from_functions(&module.functions)?;
        foundation.set_exact_types(module.meta.exact_types.clone())?;
        validate_unique_layout_definitions(
            module
                .meta
                .layouts
                .iter()
                .map(|(_, layout)| &layout.identity)
                .chain(module.meta.arrays.iter().map(|(_, array)| &array.identity)),
        )?;
        let layout_identities = module
            .meta
            .layouts
            .iter()
            .map(|(_, layout)| &layout.identity)
            .chain(module.meta.arrays.iter().map(|(_, array)| &array.identity))
            .chain(
                module
                    .meta
                    .type_descriptors
                    .iter()
                    .map(|(_, descriptor)| &descriptor.instance_layout),
            )
            .chain(
                module
                    .globals
                    .iter()
                    .filter_map(|(_, global)| match &global.init {
                        crate::GlobalInit::Storage { layout, .. } => Some(layout),
                        crate::GlobalInit::StringConst { .. }
                        | crate::GlobalInit::CString { .. } => None,
                    }),
            )
            .collect::<Vec<_>>();
        let mut layouts = BTreeMap::new();
        let mut scans = BTreeMap::new();
        for identity in &layout_identities {
            insert_projected_identity(
                &mut layouts,
                Some(identity.layout_record()),
                LirFoundationTable::Layout,
            )?;
            insert_projected_identity(
                &mut scans,
                Some(identity.scan_record()),
                LirFoundationTable::Scan,
            )?;
        }
        foundation.set_layouts(layouts.into_values().collect())?;
        foundation.set_scans(scans.into_values().collect())?;
        foundation.project_materializations(
            layout_identities,
            module
                .meta
                .type_descriptors
                .iter()
                .map(|(_, descriptor)| descriptor),
            module.globals.iter().map(|(_, global)| global),
        )?;
        foundation.project_global_identities(&module.globals)?;
        foundation.project_symbols(&module.meta.type_descriptors, &module.globals)?;
        foundation.set_dispatch_tables(
            module
                .meta
                .type_descriptors
                .iter()
                .flat_map(|(_, descriptor)| {
                    std::iter::once(descriptor.vtable.identity_record().clone()).chain(
                        descriptor
                            .itables
                            .iter()
                            .map(|itable| itable.identity_record().clone()),
                    )
                })
                .collect(),
        )?;
        foundation.set_runtime_types(
            module
                .meta
                .type_descriptors
                .iter()
                .map(|(_, descriptor)| descriptor.identity.runtime_type())
                .collect(),
        )?;
        foundation.set_c_abi_signatures(module.meta.canonical_c_abi.signatures().to_vec())?;
        foundation.set_c_abi_layouts(module.meta.canonical_c_abi.layouts().to_vec())?;
        foundation.set_native_contracts(module.meta.native_externals.contracts().to_vec())?;
        foundation.set_native_link_requirements(
            module.meta.native_externals.link_requirements().to_vec(),
        )?;
        foundation.project_generated_bridges(module)?;
        foundation.project_generated_bridge_symbols(module)?;
        Ok(foundation)
    }

    fn project_generated_bridges(
        &mut self,
        module: &Module,
    ) -> Result<(), LirFoundationBuildError> {
        let mut units = BTreeMap::new();
        let mut atoms = BTreeMap::new();
        let mut callbacks = Vec::new();

        for (_, external) in module.extern_functions.iter() {
            if let crate::ExternFunctionKind::C { bridge, .. } = &external.kind {
                insert_generated_bridge_entry(&mut units, &mut atoms, bridge)?;
            }
        }
        for (_, bridge) in module.native_global_bridges.gets.iter() {
            insert_generated_bridge_entry(&mut units, &mut atoms, &bridge.identity)?;
        }
        for (_, bridge) in module.native_global_bridges.sets.iter() {
            insert_generated_bridge_entry(&mut units, &mut atoms, &bridge.identity)?;
        }
        for (_, bridge) in module.native_global_bridges.addresses.iter() {
            insert_generated_bridge_entry(&mut units, &mut atoms, &bridge.identity)?;
        }
        for (_, bridge) in module.callback_bridges.iter() {
            insert_generated_bridge_entry(&mut units, &mut atoms, bridge.trampoline.entry())?;
        }
        for (_, bridge) in module.foreign_callback_bridges.iter() {
            insert_generated_bridge_entry(&mut units, &mut atoms, bridge.trampoline.entry())?;
            insert_projected_identity(
                &mut atoms,
                Some(bridge.trampoline.signature_descriptor_record()),
                LirFoundationTable::BridgeAtom,
            )?;
            callbacks.push(crate::CallbackBridgeRecord::new(
                bridge.application,
                bridge.trampoline.signature(),
                bridge.trampoline.entry().unit(),
            ));
        }
        let required_layouts = required_generated_bridge_layouts(
            units.iter().map(|(&unit, record)| (unit, *record.key())),
            module.meta.native_externals.contracts(),
            module.meta.canonical_c_abi.signatures(),
            module.meta.canonical_c_abi.layouts(),
        )
        .map_err(LirFoundationBuildError::GeneratedBridgeLayouts)?;
        for (unit, layouts) in required_layouts {
            for layout in layouts {
                let atom = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
                    module.cone,
                    GeneratedBridgeAtomRoleKey::StaticAssertSupport { unit, layout },
                ))
                .map_err(LirFoundationBuildError::GeneratedBridgeAtomHash)?;
                insert_projected_identity(&mut atoms, Some(&atom), LirFoundationTable::BridgeAtom)?;
            }
        }

        self.set_bridge_units(units.into_values().collect())?;
        self.set_bridge_atoms(atoms.into_values().collect())?;
        self.set_callback_bridges(callbacks)
    }

    fn project_materializations<'identity, 'descriptor, 'global>(
        &mut self,
        identities: impl IntoIterator<Item = &'identity crate::LayoutIdentity>,
        descriptors: impl IntoIterator<Item = &'descriptor crate::TypeDescriptor>,
        globals: impl IntoIterator<Item = &'global crate::Global>,
    ) -> Result<(), LirFoundationBuildError> {
        let mut groups = BTreeMap::new();
        let mut members = BTreeMap::new();
        for identity in identities {
            insert_materialization_records(
                &mut groups,
                &mut members,
                identity.lir_odr_group_record(),
                None,
            )?;
            for member in identity.odr_member_records() {
                insert_materialization_records(&mut groups, &mut members, None, Some(member))?;
            }
        }
        for descriptor in descriptors {
            insert_materialization_records(
                &mut groups,
                &mut members,
                descriptor.identity.lir_odr_group_record(),
                descriptor.identity.odr_member_record(),
            )?;
            insert_materialization_records(
                &mut groups,
                &mut members,
                descriptor.vtable.lir_odr_group_record(),
                descriptor.vtable.odr_member_record(),
            )?;
            for itable in &descriptor.itables {
                insert_materialization_records(
                    &mut groups,
                    &mut members,
                    itable.lir_odr_group_record(),
                    itable.odr_member_record(),
                )?;
            }
        }
        for global in globals {
            match &global.init {
                crate::GlobalInit::Storage { identity, .. } => {
                    insert_materialization_records(
                        &mut groups,
                        &mut members,
                        identity.lir_odr_group_record(),
                        identity.odr_member_record(),
                    )?;
                }
                crate::GlobalInit::StringConst { identity, .. } => {
                    insert_materialization_records(
                        &mut groups,
                        &mut members,
                        identity.lir_odr_group_record(),
                        identity.odr_member_record(),
                    )?;
                }
                crate::GlobalInit::CString { .. } => {}
            }
        }
        self.set_odr_groups(groups.into_values().collect())?;
        self.set_odr_members(members.into_values().collect())
    }

    fn project_global_identities(
        &mut self,
        globals: &la_arena::Arena<crate::Global>,
    ) -> Result<(), LirFoundationBuildError> {
        self.set_static_storages(
            globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    crate::GlobalInit::Storage { identity, .. } => {
                        Some(identity.identity_record().clone())
                    }
                    crate::GlobalInit::StringConst { .. } | crate::GlobalInit::CString { .. } => {
                        None
                    }
                })
                .collect(),
        )?;
        self.set_immortal_objects(
            globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    crate::GlobalInit::StringConst { identity, .. } => {
                        Some(identity.identity_record().clone())
                    }
                    crate::GlobalInit::CString { .. } | crate::GlobalInit::Storage { .. } => None,
                })
                .collect(),
        )
    }

    fn project_symbols(
        &mut self,
        descriptors: &la_arena::Arena<crate::TypeDescriptor>,
        globals: &la_arena::Arena<crate::Global>,
    ) -> Result<(), LirFoundationBuildError> {
        let requests = self
            .symbol_requests
            .requests()
            .iter()
            .copied()
            .chain(
                descriptors
                    .iter()
                    .map(|(_, descriptor)| descriptor.identity.symbol_request()),
            )
            .chain(
                globals
                    .iter()
                    .filter_map(|(_, global)| global.persistent_symbol_request()),
            )
            .collect();
        self.set_symbol_requests(
            scoop_identity::PersistentSymbolRequestTable::new(requests)
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        Ok(())
    }

    fn project_generated_bridge_symbols(
        &mut self,
        module: &Module,
    ) -> Result<(), LirFoundationBuildError> {
        let bridge_requests = module
            .extern_functions
            .iter()
            .filter_map(|(_, external)| match &external.kind {
                crate::ExternFunctionKind::C { bridge, .. } => Some(bridge.symbol_request()),
                crate::ExternFunctionKind::Scoop { .. } => None,
            })
            .chain(
                module
                    .native_global_bridges
                    .gets
                    .iter()
                    .map(|(_, bridge)| bridge.identity.symbol_request()),
            )
            .chain(
                module
                    .native_global_bridges
                    .sets
                    .iter()
                    .map(|(_, bridge)| bridge.identity.symbol_request()),
            )
            .chain(
                module
                    .native_global_bridges
                    .addresses
                    .iter()
                    .map(|(_, bridge)| bridge.identity.symbol_request()),
            )
            .chain(
                module
                    .callback_bridges
                    .iter()
                    .map(|(_, bridge)| bridge.trampoline.entry().symbol_request()),
            )
            .chain(
                module
                    .foreign_callback_bridges
                    .iter()
                    .flat_map(|(_, bridge)| {
                        [
                            bridge.trampoline.entry().symbol_request(),
                            bridge.trampoline.signature_descriptor_symbol_request(),
                        ]
                    }),
            );
        let requests = self
            .symbol_requests
            .requests()
            .iter()
            .copied()
            .chain(bridge_requests)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.set_symbol_requests(
            scoop_identity::PersistentSymbolRequestTable::new(requests)
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        Ok(())
    }

    /// Materialize the complete strong-only definition graph after the
    /// `SingleConeStrong` ODR gate has accepted the module.
    pub(super) fn project_strong_definitions(
        &mut self,
        module: &Module,
    ) -> Result<(), LirFoundationBuildError> {
        let producer = module.cone;
        let mut plans = BTreeMap::new();
        let mut atoms = BTreeMap::new();
        let mut symbols = BTreeSet::new();
        let mut cstrings_by_owner = BTreeMap::new();
        for (_, global) in module.globals.iter() {
            let crate::GlobalInit::CString { identity, .. } = &global.init else {
                continue;
            };
            cstrings_by_owner
                .entry(identity.owner())
                .or_insert_with(Vec::new)
                .push(identity);
        }

        for function in &module.functions {
            let body = function.callable_body.id();
            let cstrings = cstrings_by_owner.remove(&body).unwrap_or_default();
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
                callable_body_associated_atoms(function, &cstrings),
            )?;
            for identity in cstrings {
                if atoms.get(&identity.atom_record().id()) != Some(identity.atom_record()) {
                    return Err(LirFoundationBuildError::CallableCStringAtomMismatch {
                        owner: *identity.owner().as_array(),
                        atom: *identity.atom_record().id().as_array(),
                    });
                }
            }
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableRegistration,
                Vec::new(),
            )?;
            for safepoint in function.safepoints.iter() {
                insert_strong_definition(
                    &mut plans,
                    &mut atoms,
                    &mut symbols,
                    producer,
                    StrongDefinitionEntity::safepoint_site(safepoint.site_id()),
                    StrongDefinitionRole::SafepointRegistration,
                    Vec::new(),
                )?;
            }
        }
        if let Some((owner, _)) = cstrings_by_owner.into_iter().next() {
            return Err(LirFoundationBuildError::CallableCStringOwnerMissing {
                owner: *owner.as_array(),
            });
        }

        for (_, descriptor) in module.meta.type_descriptors.iter() {
            let exact = descriptor.identity.exact_type();
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
                vec![(
                    DefinitionAtomRole::AddressTakenConstant,
                    DefinitionAtomSubkey::ExactType(exact),
                )],
            )?;
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
                Vec::new(),
            )?;
        }
        for layout in &self.layouts {
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::layout(layout.id()),
                StrongDefinitionRole::Layout,
                Vec::new(),
            )?;
        }
        for scan in &self.scans {
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::scan(scan.id()),
                StrongDefinitionRole::ScanProgram,
                Vec::new(),
            )?;
        }
        for table in &self.dispatch_tables {
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::dispatch_table(table.id()),
                StrongDefinitionRole::DispatchTable,
                Vec::new(),
            )?;
        }

        let storage_semantics = crate::StrongStaticStorageSemanticPlanSetV1::from_module(module)
            .map_err(LirFoundationBuildError::StaticStorageSemantics)?;
        for storage in storage_semantics.storages() {
            let storage_id = storage.storage();
            let mut associated = Vec::new();
            if let crate::StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                immortal_relocations,
                ..
            } = storage.initial_state()
            {
                associated.push((
                    DefinitionAtomRole::AddressTakenConstant,
                    DefinitionAtomSubkey::StaticStorage(storage_id),
                ));
                if !immortal_relocations.is_empty() {
                    associated.push((
                        DefinitionAtomRole::RuntimeRecord,
                        DefinitionAtomSubkey::StaticStorage(storage_id),
                    ));
                }
            }
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::static_storage(storage_id),
                StrongDefinitionRole::StaticStorage,
                associated,
            )?;
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::static_storage(storage_id),
                StrongDefinitionRole::RootRegistration,
                Vec::new(),
            )?;
        }

        for object in &self.immortal_objects {
            let object_id = object.id();
            for role in [
                StrongDefinitionRole::ImmortalObject,
                StrongDefinitionRole::ImmortalRegistration,
            ] {
                insert_strong_definition(
                    &mut plans,
                    &mut atoms,
                    &mut symbols,
                    producer,
                    StrongDefinitionEntity::immortal_object(object_id),
                    role,
                    Vec::new(),
                )?;
            }
        }

        for (_, unit) in module.initialization_units.iter() {
            let unit_id = unit.identity.id();
            let entity = StrongDefinitionEntity::initialization_unit(unit_id);
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                entity,
                StrongDefinitionRole::InitializationCell,
                Vec::new(),
            )?;
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                entity,
                StrongDefinitionRole::InitializationDescriptor,
                vec![(
                    DefinitionAtomRole::AddressTakenConstant,
                    DefinitionAtomSubkey::InitializationUnit(unit_id),
                )],
            )?;
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                entity,
                StrongDefinitionRole::InitializationRegistration,
                Vec::new(),
            )?;
        }

        for atom in &self.bridge_atoms {
            if !atom.key().atom().is_materializable() {
                continue;
            }
            let entity = StrongDefinitionEntity::generated_bridge_atom(atom.key())
                .map_err(LirFoundationBuildError::DefinitionIdentity)?;
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                entity,
                StrongDefinitionRole::GeneratedBridge,
                Vec::new(),
            )?;
        }

        let image_support = [
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateGroup,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateName,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateVersion,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Dependencies,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::StaticStorages,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::ImmortalObjects,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::InitializationUnits,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::TypeRegistrations,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Safepoints,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Callables,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArrayBoundsMessage,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArraySizeOverflowMessage,
            ),
        ]
        .into_iter()
        .map(|(role, support)| (role, DefinitionAtomSubkey::ConeImageSupport(support)))
        .collect();
        insert_strong_definition(
            &mut plans,
            &mut atoms,
            &mut symbols,
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
            image_support,
        )?;

        if matches!(module.output, crate::LirOutput::Executable { .. }) {
            insert_strong_definition(
                &mut plans,
                &mut atoms,
                &mut symbols,
                producer,
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
                Vec::new(),
            )?;
        }

        self.set_definition_plans(plans.into_values().collect())?;
        self.set_definition_atoms(atoms.into_values().collect())?;
        self.set_symbol_requests(
            scoop_identity::PersistentSymbolRequestTable::new(symbols.into_iter().collect())
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        Ok(())
    }

    fn from_functions(functions: &[Function]) -> Result<Self, LirFoundationBuildError> {
        let mut callable_bodies = Vec::with_capacity(functions.len());
        let safepoint_count = functions
            .iter()
            .map(|function| function.safepoints.len())
            .sum();
        let mut safepoint_sites = Vec::with_capacity(safepoint_count);
        let mut safepoints = Vec::with_capacity(safepoint_count);
        let mut symbol_requests = Vec::with_capacity(functions.len());

        for (function_index, function) in functions.iter().enumerate() {
            let callable_body = function.callable_body.id();
            callable_bodies.push(function.callable_body.identity_record().clone());
            symbol_requests.push(function.callable_body.symbol_request());
            for identity in function.safepoints.iter() {
                if identity.owner() != callable_body {
                    return Err(LirFoundationBuildError::SafepointOwnerMismatch {
                        function: function_index,
                        site: *identity.site_id().as_array(),
                        expected: *callable_body.as_array(),
                        actual: *identity.owner().as_array(),
                    });
                }
                safepoint_sites.push(identity.site_record().clone());
                safepoints.push(SafepointMappingRecord::from_identity(identity));
            }
        }

        let mut foundation = Self::empty();
        foundation.set_callable_bodies(callable_bodies)?;
        foundation.set_safepoint_sites(safepoint_sites)?;
        foundation.set_safepoints(safepoints)?;
        foundation.set_symbol_requests(
            scoop_identity::PersistentSymbolRequestTable::new(symbol_requests)
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        Ok(foundation)
    }
}

fn callable_body_associated_atoms(
    function: &Function,
    cstrings: &[&crate::CallableCStringIdentity],
) -> Vec<(DefinitionAtomRole, DefinitionAtomSubkey)> {
    let body = function.callable_body.id();
    let subkey = || DefinitionAtomSubkey::CallableBody(body);
    let mut associated = vec![(DefinitionAtomRole::CompactUnwind, subkey())];
    if !function.safepoints.is_empty() {
        associated.push((DefinitionAtomRole::Stackmap, subkey()));
    }
    if function.blocks.iter().any(|(_, block)| {
        block
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, crate::Instruction::Invoke { .. }))
    }) {
        associated.push((DefinitionAtomRole::Lsda, subkey()));
        associated.push((DefinitionAtomRole::EhFrame, subkey()));
    }
    associated.extend(cstrings.iter().map(|identity| {
        (
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::StructuralPath(identity.path().clone()),
        )
    }));
    associated
}

#[allow(clippy::too_many_arguments)]
fn insert_strong_definition(
    plans: &mut BTreeMap<ObjectDefinitionPlanId, DefinitionPlanRecord>,
    atoms: &mut BTreeMap<ObjectDefinitionAtomId, DefinitionAtomRecord>,
    symbols: &mut BTreeSet<PersistentSymbolRequest>,
    producer: scoop_identity::ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
    associated: Vec<(DefinitionAtomRole, DefinitionAtomSubkey)>,
) -> Result<(), LirFoundationBuildError> {
    let key = ObjectDefinitionPlanKey::strong(producer, entity, role)
        .map_err(LirFoundationBuildError::DefinitionIdentity)?;
    let Some(symbol_key) = key.primary_symbol_key() else {
        return Err(LirFoundationBuildError::DefinitionIdentity(
            ObjectDefinitionIdentityError::StrongRoleEntityMismatch,
        ));
    };
    symbols.insert(
        PersistentSymbolRequest::new(symbol_key, LinkageClass::ConeStrong)
            .map_err(LirFoundationBuildError::SymbolRequest)?,
    );
    let plan =
        CborIdentityRecord::from_key(key).map_err(LirFoundationBuildError::DefinitionHash)?;
    let plan_id = plan.id();
    insert_projected_identity(plans, Some(&plan), LirFoundationTable::DefinitionPlan)?;

    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan_id,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .map_err(LirFoundationBuildError::DefinitionHash)?;
    insert_projected_identity(atoms, Some(&primary), LirFoundationTable::DefinitionAtom)?;
    for (atom_role, subkey) in associated {
        let atom =
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(plan_id, atom_role, subkey))
                .map_err(LirFoundationBuildError::DefinitionHash)?;
        insert_projected_identity(atoms, Some(&atom), LirFoundationTable::DefinitionAtom)?;
    }
    Ok(())
}

fn insert_generated_bridge_entry(
    units: &mut BTreeMap<GeneratedBridgeUnitId, BridgeUnitRecord>,
    atoms: &mut BTreeMap<GeneratedBridgeAtomId, BridgeAtomRecord>,
    entry: &crate::GeneratedBridgeEntryIdentity,
) -> Result<(), LirFoundationBuildError> {
    insert_projected_identity(
        units,
        Some(entry.unit_record()),
        LirFoundationTable::BridgeUnit,
    )?;
    insert_projected_identity(
        atoms,
        Some(entry.primary_record()),
        LirFoundationTable::BridgeAtom,
    )
}

fn insert_materialization_records(
    groups: &mut BTreeMap<OdrGroupId, OdrGroupRecord>,
    members: &mut BTreeMap<OdrMemberId, OdrMemberRecord>,
    group: Option<&OdrGroupRecord>,
    member: Option<&OdrMemberRecord>,
) -> Result<(), LirFoundationBuildError> {
    insert_projected_identity(groups, group, LirFoundationTable::OdrGroup)?;
    insert_projected_identity(members, member, LirFoundationTable::OdrMember)
}

fn validate_unique_layout_definitions<'a>(
    identities: impl IntoIterator<Item = &'a crate::LayoutIdentity>,
) -> Result<(), LirFoundationBuildError> {
    let mut defined_layouts = BTreeSet::new();
    for identity in identities {
        let id = identity.layout_record().id();
        if !defined_layouts.insert(id) {
            return Err(LirFoundationBuildError::DuplicateIdentity {
                table: LirFoundationTable::Layout,
                identity: *id.as_array(),
            });
        }
    }
    Ok(())
}

fn insert_projected_identity<I, K>(
    records: &mut BTreeMap<I, CborIdentityRecord<I, K>>,
    record: Option<&CborIdentityRecord<I, K>>,
    table: LirFoundationTable,
) -> Result<(), LirFoundationBuildError>
where
    I: Copy + Ord + PersistentId,
    K: Clone + Eq,
{
    let Some(record) = record else {
        return Ok(());
    };
    match records.entry(record.id()) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(record.clone());
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == record => Ok(()),
        std::collections::btree_map::Entry::Occupied(entry) => {
            Err(LirFoundationBuildError::IdentityCollision {
                table,
                identity: *entry.key().as_array(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner,
        InitializationUnitKey, NonEmptyVec, OdrMemberRole, PackagePath, PersistentExactTypeId,
        PersistentExtensionPropertyId, PersistentFunctionId, PersistentInitializationUnitId,
        SafepointSiteRole, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
        SpecializationKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
        StructuralPathSegment,
    };

    use super::*;
    use crate::{
        AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CallingConvention,
        GcEffect, Global, GlobalInit, ImmortalObjectIdentity, Instruction, InvokeSite,
        ItableRecord, LayoutIdentity, LirStaticInitialState, LirTargetProfile, MANAGED_PTR,
        MaterializationRoot, NoGcCallDestination, NoGcInvokeSite, NoGcRuntimeFunction,
        NoGcTypedCall, PointerKind, RefScan, RuntimeTypeMappingRecord, SafepointIdentities,
        SafepointIdentity, SafepointSiteRef, ScoopAbiSignature, StaticStorageIdentity, Terminator,
        TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1,
        VoidCallSignature, VtableRecord,
    };

    #[test]
    fn projects_only_lir_first_layout_groups_and_all_new_layout_members() {
        let structural = exact_tuple(0);
        let inherited = exact_tuple(1);
        let inherited_group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: inherited,
        })
        .unwrap();
        let source = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let identities = [
            LayoutIdentity::managed_value(
                structural,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::lir_structural_odr(structural).unwrap(),
            )
            .unwrap(),
            LayoutIdentity::c_value(
                structural,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::lir_structural_odr(structural).unwrap(),
            )
            .unwrap(),
            LayoutIdentity::managed_value(
                inherited,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::prior_stage_odr(inherited_group.id()),
            )
            .unwrap(),
            LayoutIdentity::managed_value(
                source,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
        ];
        let mut foundation = CanonicalLirFoundation::empty();

        foundation
            .project_materializations(&identities, std::iter::empty(), std::iter::empty())
            .unwrap();

        assert_eq!(foundation.odr_groups.len(), 1);
        assert_eq!(
            foundation.odr_groups[0].key(),
            &SpecializationKey::StructuralType {
                exact_type: structural,
            }
        );
        assert_eq!(foundation.odr_members.len(), 6);
        assert_eq!(
            foundation
                .odr_members
                .iter()
                .filter(|member| member.key().role() == OdrMemberRole::Layout)
                .count(),
            3
        );
        assert_eq!(
            foundation
                .odr_members
                .iter()
                .filter(|member| member.key().role() == OdrMemberRole::ScanProgram)
                .count(),
            3
        );
        assert!(foundation.odr_members.iter().all(|member| {
            member.key().group() == foundation.odr_groups[0].id()
                || member.key().group() == inherited_group.id()
        }));
    }

    #[test]
    fn rejects_two_physical_layout_definitions_with_one_identity() {
        let exact_type = exact_tuple(0);
        let identity = LayoutIdentity::managed_value(
            exact_type,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::lir_structural_odr(exact_type).unwrap(),
        )
        .unwrap();
        let identities = [identity.clone(), identity];

        let error = validate_unique_layout_definitions(&identities).unwrap_err();

        assert!(matches!(
            error,
            LirFoundationBuildError::DuplicateIdentity {
                table: LirFoundationTable::Layout,
                ..
            }
        ));
    }

    #[test]
    fn projects_descriptor_and_dispatch_members_under_one_exact_type_root() {
        let interface_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let owner_exact = exact_tuple(0);
        let owner_root = MaterializationRoot::lir_structural_odr(owner_exact).unwrap();
        let mut descriptors = Arena::new();
        let interface_identity = TypeDescriptorIdentity::new(
            RuntimeTypeMappingRecord::new(interface_exact).unwrap(),
            MaterializationRoot::cone_owned(),
        )
        .unwrap();
        let interface_vtable = VtableRecord::new(&interface_identity, Vec::new()).unwrap();
        let interface = descriptors.alloc(TypeDescriptor {
            diagnostic_name: "Interface".to_string(),
            identity: interface_identity,
            instance_layout: LayoutIdentity::managed_object(
                interface_exact,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            instance_shape: TypeInstanceShapeV1::abstract_ref(),
            inline_scan: crate::TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable: interface_vtable,
            itables: Vec::new(),
        });
        let owner_identity = TypeDescriptorIdentity::new(
            RuntimeTypeMappingRecord::new(owner_exact).unwrap(),
            owner_root.clone(),
        )
        .unwrap();
        let owner_vtable = VtableRecord::new(&owner_identity, Vec::new()).unwrap();
        let owner_itables = vec![
            ItableRecord::new(
                &owner_identity,
                interface_exact,
                TypeDescriptorRef::Local(interface),
                Vec::new(),
            )
            .unwrap(),
        ];
        descriptors.alloc(TypeDescriptor {
            diagnostic_name: "Owner".to_string(),
            identity: owner_identity,
            instance_layout: LayoutIdentity::managed_object(
                owner_exact,
                LirTargetProfile::DARWIN_AARCH64,
                owner_root,
            )
            .unwrap(),
            instance_shape: TypeInstanceShapeV1::abstract_ref(),
            inline_scan: crate::TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable: owner_vtable,
            itables: owner_itables,
        });
        let mut foundation = CanonicalLirFoundation::empty();

        foundation
            .project_materializations(
                std::iter::empty(),
                descriptors.iter().map(|(_, descriptor)| descriptor),
                std::iter::empty(),
            )
            .unwrap();
        foundation
            .project_symbols(&descriptors, &Arena::new())
            .unwrap();

        assert_eq!(foundation.odr_groups.len(), 1);
        assert_eq!(foundation.odr_members.len(), 3);
        assert_eq!(
            foundation
                .odr_members
                .iter()
                .filter(|member| member.key().role() == OdrMemberRole::TypeDescriptor)
                .count(),
            1
        );
        assert_eq!(
            foundation
                .odr_members
                .iter()
                .filter(|member| member.key().role() == OdrMemberRole::DispatchTable)
                .count(),
            2
        );
        assert!(
            foundation
                .odr_members
                .iter()
                .all(|member| member.key().group() == foundation.odr_groups[0].id())
        );
        assert_eq!(foundation.symbol_requests.requests().len(), 2);
        assert!(
            foundation
                .symbol_requests
                .requests()
                .iter()
                .all(|request| matches!(
                    request.key(),
                    scoop_identity::PersistentSymbolKey::TypeDescriptor(_)
                ))
        );
    }

    #[test]
    fn inherited_delegated_globals_emit_members_without_reemitting_the_group() {
        let receiver = exact_tuple(0);
        let declaration = SourceDeclarationKey::extension_property(
            source_site(),
            CanonicalIdentifier::new("memoized").unwrap(),
            1,
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        );
        let property =
            PersistentExtensionPropertyId::from_source_declaration(&declaration).unwrap();
        let receiver_arguments = NonEmptyVec::from_first(receiver, []);
        let unit_key = InitializationUnitKey::GenericDelegatedExtensionApplication {
            property,
            receiver_arguments: receiver_arguments.clone(),
        };
        let unit = PersistentInitializationUnitId::from_key(&unit_key).unwrap();
        let group = OdrGroupId::from_key(&SpecializationKey::DelegatedProperty {
            origin: property,
            receiver_arguments,
        })
        .unwrap();
        let root = MaterializationRoot::prior_stage_odr(group);
        let storage =
            StaticStorageIdentity::initialization_failure_root(unit, root.clone()).unwrap();
        let object_key = ImmortalObjectKey::string_constant(
            ImmortalObjectOwner::InitializationUnit(unit),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
                [],
            ),
        );
        let object = ImmortalObjectIdentity::from_key(object_key, root).unwrap();
        let mut globals = Arena::new();
        globals.alloc(Global {
            address_kind: PointerKind::Raw,
            scan: RefScan::References(vec![0]),
            init: GlobalInit::Storage {
                identity: storage,
                layout: LayoutIdentity::managed_value(
                    exact_tuple(0),
                    LirTargetProfile::DARWIN_AARCH64,
                    MaterializationRoot::cone_owned(),
                )
                .unwrap(),
                ty: MANAGED_PTR,
                initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
                thread_local: false,
            },
        });
        globals.alloc(Global {
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::StringConst {
                identity: object,
                value: "text".to_string(),
            },
        });
        let mut foundation = CanonicalLirFoundation::empty();

        foundation
            .project_materializations(
                std::iter::empty(),
                std::iter::empty(),
                globals.iter().map(|(_, global)| global),
            )
            .unwrap();
        foundation.project_symbols(&Arena::new(), &globals).unwrap();

        assert!(foundation.odr_groups.is_empty());
        assert_eq!(foundation.odr_members.len(), 2);
        assert!(
            foundation
                .odr_members
                .iter()
                .all(|member| member.key().group() == group)
        );
        assert!(
            foundation
                .odr_members
                .iter()
                .any(|member| member.key().role() == OdrMemberRole::StaticStorage)
        );
        assert!(
            foundation
                .odr_members
                .iter()
                .any(|member| member.key().role() == OdrMemberRole::ImmortalObject)
        );
        assert_eq!(foundation.symbol_requests.requests().len(), 2);
        assert!(
            foundation
                .symbol_requests
                .requests()
                .iter()
                .all(|request| request.linkage() == scoop_identity::LinkageClass::OdrWeak)
        );
    }

    #[test]
    fn projects_callable_bodies_and_safepoint_relations_without_block_scanning() {
        let first = callable_body("first");
        let first_site =
            SafepointIdentity::new(first.id(), SafepointSiteRole::NativeSafeTransition, 0).unwrap();
        let second_site =
            SafepointIdentity::new(first.id(), SafepointSiteRole::ManagedPoll, 0).unwrap();
        let second = callable_body("second");
        let functions = vec![
            function(
                first,
                SafepointIdentities::checked(vec![
                    (SafepointSiteRef::from_u32(4), second_site.clone()),
                    (SafepointSiteRef::from_u32(2), first_site.clone()),
                ])
                .unwrap(),
            ),
            function(second, SafepointIdentities::default()),
        ];

        let foundation = CanonicalLirFoundation::from_functions(&functions).unwrap();

        assert_eq!(foundation.callable_bodies.len(), 2);
        assert_eq!(foundation.safepoint_sites.len(), 2);
        assert_eq!(foundation.safepoints.len(), 2);
        assert_eq!(foundation.symbol_requests.requests().len(), 2);
        assert!(
            foundation
                .symbol_requests
                .requests()
                .iter()
                .all(|request| request.linkage() == scoop_identity::LinkageClass::ConeStrong)
        );
        assert!(
            foundation
                .symbol_requests
                .requests()
                .iter()
                .all(|request| matches!(
                    request.key(),
                    scoop_identity::PersistentSymbolKey::CallableBody(_)
                ))
        );
        let mut expected = vec![first_site, second_site];
        expected.sort_by_key(SafepointIdentity::site_id);
        for ((site, mapping), identity) in foundation
            .safepoint_sites
            .iter()
            .zip(&foundation.safepoints)
            .zip(expected)
        {
            assert_eq!(site.id(), identity.site_id());
            assert_eq!(mapping.site(), identity.site_id());
            assert_eq!(mapping.safepoint(), identity.runtime_id());
        }
    }

    #[test]
    fn projects_callable_backend_associated_atoms_from_complete_lir() {
        let body = callable_body("physicalAtoms");
        let site = SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 0).unwrap();
        let mut function = function(
            body.clone(),
            SafepointIdentities::checked(vec![(SafepointSiteRef::from_u32(0), site)]).unwrap(),
        );

        assert_eq!(
            callable_body_associated_atoms(&function, &[]),
            vec![
                (
                    DefinitionAtomRole::CompactUnwind,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
                (
                    DefinitionAtomRole::Stackmap,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
            ]
        );

        let signature = function
            .call_targets
            .void_signatures
            .alloc(VoidCallSignature::new(Vec::new(), CallingConvention::Cdecl));
        let target = function.call_targets.no_gc_targets.void.alloc(CallTarget {
            destination: NoGcCallDestination::runtime(NoGcRuntimeFunction::Rethrow),
            signature,
        });
        let normal = function.blocks.alloc(BasicBlock {
            name: "normal".to_string(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        });
        let unwind = function.blocks.alloc(BasicBlock {
            name: "unwind".to_string(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        });
        function.blocks[function.entry]
            .instructions
            .push(Instruction::Invoke {
                site: InvokeSite::NoGc(NoGcInvokeSite {
                    call: NoGcTypedCall::Void {
                        target,
                        args: Vec::new(),
                    },
                    normal,
                    unwind,
                }),
            });

        assert_eq!(
            callable_body_associated_atoms(&function, &[]),
            vec![
                (
                    DefinitionAtomRole::CompactUnwind,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
                (
                    DefinitionAtomRole::Stackmap,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
                (
                    DefinitionAtomRole::Lsda,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
                (
                    DefinitionAtomRole::EhFrame,
                    DefinitionAtomSubkey::CallableBody(body.id()),
                ),
            ]
        );
    }

    #[test]
    fn rejects_a_site_owned_by_another_callable_body() {
        let function_body = callable_body("function");
        let other_body = callable_body("other");
        let site =
            SafepointIdentity::new(other_body.id(), SafepointSiteRole::ManagedCall, 0).unwrap();
        let functions = vec![function(
            function_body,
            SafepointIdentities::checked(vec![(SafepointSiteRef::from_u32(0), site)]).unwrap(),
        )];

        let error = CanonicalLirFoundation::from_functions(&functions).unwrap_err();

        assert!(matches!(
            error,
            LirFoundationBuildError::SafepointOwnerMismatch { function: 0, .. }
        ));
    }

    #[test]
    fn rejects_a_callable_body_claimed_by_two_functions() {
        let body = callable_body("duplicate");
        let functions = vec![
            function(body.clone(), SafepointIdentities::default()),
            function(body, SafepointIdentities::default()),
        ];

        let error = CanonicalLirFoundation::from_functions(&functions).unwrap_err();

        assert!(matches!(
            error,
            LirFoundationBuildError::DuplicateIdentity {
                table: LirFoundationTable::CallableBody,
                ..
            }
        ));
    }

    #[test]
    fn projects_immortal_string_object_identities_from_globals() {
        let owner = ImmortalObjectOwner::Callable(CallableMaterialization::new(
            CallableTemplateOwner::Function(function_id("stringOwner")),
            CallableMaterializationContext::NoSubstitution,
        ));
        let key = ImmortalObjectKey::string_constant(
            owner,
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
                [],
            ),
        );
        let identity =
            ImmortalObjectIdentity::from_key(key.clone(), MaterializationRoot::cone_owned())
                .unwrap();
        let expected = identity.identity_record().clone();
        let mut globals = Arena::new();
        globals.alloc(Global {
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::StringConst {
                identity: identity.clone(),
                value: "text".to_string(),
            },
        });

        let mut foundation = CanonicalLirFoundation::empty();
        foundation.project_global_identities(&globals).unwrap();
        foundation.project_symbols(&Arena::new(), &globals).unwrap();

        assert_eq!(foundation.immortal_objects, vec![expected]);
        assert!(foundation.static_storages.is_empty());
        assert_eq!(foundation.immortal_objects[0].key(), &key);
        assert_eq!(foundation.symbol_requests.requests().len(), 1);
        assert_eq!(
            foundation.symbol_requests.requests()[0].linkage(),
            scoop_identity::LinkageClass::ConeStrong
        );
        assert_eq!(
            foundation.symbol_requests.requests()[0].key(),
            scoop_identity::PersistentSymbolKey::ImmortalObject(
                foundation.immortal_objects[0].id()
            )
        );
    }

    fn function(callable_body: CallableBodyIdentity, safepoints: SafepointIdentities) -> Function {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        });
        Function {
            callable_body,
            gc_effect: GcEffect::Managed,
            signature: ScoopAbiSignature::new(
                Vec::new(),
                AbiReturn::UnitVoid,
                CallingConvention::Cdecl,
            ),
            call_targets: CallTargets::default(),
            safepoints,
            locals: Arena::new(),
            temps: Arena::new(),
            blocks,
            entry,
        }
    }

    fn exact_tuple(arity: usize) -> PersistentExactTypeId {
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(NonEmptyVec::from_first(
            unit,
            vec![unit; arity],
        )))
        .unwrap()
    }

    fn callable_body(name: &str) -> CallableBodyIdentity {
        CallableBodyIdentity::for_function(function_id(name)).unwrap()
    }

    fn function_id(name: &str) -> PersistentFunctionId {
        let declaration = SourceDeclarationKey::function(
            source_site(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        PersistentFunctionId::from_source_declaration(&declaration).unwrap()
    }

    fn source_site() -> SourceDeclarationSite {
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap()
    }
}
