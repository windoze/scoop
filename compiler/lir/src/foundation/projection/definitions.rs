//! Physical definitions and associated atoms for the complete LIR output.

use super::*;

mod writer;
use writer::DefinitionWriter;

impl CanonicalLirFoundation {
    /// Project each emitted definition under its actual Strong or ODR owner.
    pub(in crate::foundation) fn project_definitions(
        &mut self,
        module: &Module,
    ) -> Result<(), LirFoundationBuildError> {
        let producer = module.cone;
        let callable_runtime_scans = crate::StrongCallableRuntimeScanPlanSetV1::from_module(module)
            .map_err(LirFoundationBuildError::CallableRuntimeScans)?;
        let mut writer = DefinitionWriter::new(module, &self.odr_members)?;
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

        for function in module.callable_bodies() {
            let body = function.callable_body.id();
            let cstrings = cstrings_by_owner.remove(&body).unwrap_or_default();
            let runtime_scans = callable_runtime_scans
                .callable(body)
                .expect("the runtime-scan plan was projected from every function");
            writer.define(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
                callable_body_associated_atoms(function, &cstrings, runtime_scans.atoms()),
            )?;
            for identity in cstrings {
                if writer.atoms.get(&identity.atom_record().id()) != Some(identity.atom_record()) {
                    return Err(LirFoundationBuildError::CallableCStringAtomMismatch {
                        owner: *identity.owner().as_array(),
                        atom: *identity.atom_record().id().as_array(),
                    });
                }
            }
            writer.define(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableRegistration,
                Vec::new(),
            )?;
            for safepoint in function.safepoints.iter() {
                writer.define(
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
            let mut associated = vec![(
                DefinitionAtomRole::AddressTakenConstant,
                DefinitionAtomSubkey::ExactType(exact),
            )];
            if !descriptor.itables.is_empty() {
                associated.push((
                    DefinitionAtomRole::RuntimeRecord,
                    DefinitionAtomSubkey::ExactType(exact),
                ));
            }
            writer.define(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
                associated,
            )?;
            writer.define(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
                Vec::new(),
            )?;
        }
        for layout in &self.layouts {
            writer.define(
                StrongDefinitionEntity::layout(layout.id()),
                StrongDefinitionRole::Layout,
                Vec::new(),
            )?;
        }
        for scan in &self.scans {
            writer.define(
                StrongDefinitionEntity::scan(scan.id()),
                StrongDefinitionRole::ScanProgram,
                Vec::new(),
            )?;
        }
        for table in &self.dispatch_tables {
            writer.define(
                StrongDefinitionEntity::dispatch_table(table.id()),
                StrongDefinitionRole::DispatchTable,
                Vec::new(),
            )?;
        }

        for (_, global) in module.globals.iter() {
            if let crate::GlobalInit::RawStorage {
                identity,
                thread_local,
                ..
            } = &global.init
            {
                let storage = identity.identity_record().id();
                let associated = if *thread_local {
                    vec![(
                        DefinitionAtomRole::AddressTakenConstant,
                        DefinitionAtomSubkey::StaticStorage(storage),
                    )]
                } else {
                    Vec::new()
                };
                writer.define(
                    StrongDefinitionEntity::static_storage(storage),
                    StrongDefinitionRole::StaticStorage,
                    associated,
                )?;
            }
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
            writer.define(
                StrongDefinitionEntity::static_storage(storage_id),
                StrongDefinitionRole::StaticStorage,
                associated,
            )?;
            writer.define(
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
                writer.define(
                    StrongDefinitionEntity::immortal_object(object_id),
                    role,
                    Vec::new(),
                )?;
            }
        }

        for (_, unit) in module.initialization_units.iter() {
            let unit_id = unit.identity.id();
            let entity = StrongDefinitionEntity::initialization_unit(unit_id);
            writer.define(entity, StrongDefinitionRole::InitializationCell, Vec::new())?;
            writer.define(
                entity,
                StrongDefinitionRole::InitializationRegistration,
                vec![(
                    DefinitionAtomRole::AddressTakenConstant,
                    DefinitionAtomSubkey::InitializationUnit(unit_id),
                )],
            )?;
        }

        for atom in &self.bridge_atoms {
            if !atom.key().atom().is_materializable() {
                continue;
            }
            let entity = StrongDefinitionEntity::generated_bridge_atom(atom.key())
                .map_err(LirFoundationBuildError::DefinitionIdentity)?;
            writer.define(entity, StrongDefinitionRole::GeneratedBridge, Vec::new())?;
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
        writer.define(
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
            image_support,
        )?;

        if matches!(module.output, crate::LirOutput::Executable { .. }) {
            writer.define(
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
                Vec::new(),
            )?;
        }

        self.set_odr_members(writer.members.into_values().collect())?;
        self.set_definition_plans(writer.plans.into_values().collect())?;
        self.set_definition_atoms(writer.atoms.into_values().collect())?;
        self.set_symbol_requests(
            scoop_identity::PersistentSymbolRequestTable::new(writer.symbols.into_iter().collect())
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        Ok(())
    }
}

pub(super) fn callable_body_associated_atoms(
    function: &Function,
    cstrings: &[&crate::CallableCStringIdentity],
    runtime_scans: &[crate::StrongCallableRuntimeScanAtomV1],
) -> Vec<(DefinitionAtomRole, DefinitionAtomSubkey)> {
    let body = function.callable_body.id();
    let subkey = || DefinitionAtomSubkey::CallableBody(body);
    let mut associated = Vec::new();
    let context_keys = crate::function_context_keys(function);
    if !context_keys.is_empty() {
        associated.push((
            DefinitionAtomRole::ContextKeyTable,
            DefinitionAtomSubkey::Singleton,
        ));
        associated.extend(context_keys.into_iter().map(|key| {
            (
                DefinitionAtomRole::ContextKeyCell,
                DefinitionAtomSubkey::ExactType(key.0),
            )
        }));
    }
    if function.callable_body.release_owner().is_none() {
        associated.push((DefinitionAtomRole::CompactUnwind, subkey()));
    }
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
    associated.extend(runtime_scans.iter().enumerate().map(|(ordinal, _)| {
        (
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::StructuralPath(StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(
                    StructuralDefinitionSiteRole::SyntheticValue,
                    u32::try_from(ordinal)
                        .expect("runtime-scan planning already bounded every ordinal"),
                ),
                [],
            )),
        )
    }));
    associated
}
