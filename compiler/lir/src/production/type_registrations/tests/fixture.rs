//! Complete physical type registration fixtures.

use super::*;

#[derive(Clone, Copy, Default)]
pub(super) struct Options {
    pub(super) omit_last_registration: bool,
    pub(super) omit_first_runtime_type: bool,
    pub(super) omit_first_layout: bool,
    pub(super) omit_registration_symbol: bool,
    pub(super) omit_descriptor_symbol: bool,
    pub(super) omit_layout_symbol: bool,
    pub(super) registration_object_input: bool,
    pub(super) registration_object_patch: bool,
    pub(super) omit_descriptor_input: bool,
    pub(super) omit_registration_patch: bool,
    pub(super) omit_descriptor_patch: bool,
    pub(super) omit_layout_patch: bool,
    pub(super) omit_descriptor_diagnostic: bool,
    pub(super) extra_descriptor_atom: bool,
    pub(super) first_type_has_itable: bool,
    pub(super) first_itable_interface: Option<PersistentExactTypeId>,
    pub(super) omit_itable_directory: bool,
}

pub(super) struct TypeArtifacts {
    pub(super) exact_type: PersistentExactTypeId,
    pub(super) layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    pub(super) scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    pub(super) vtable: CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
    pub(super) descriptor_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) descriptor_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) descriptor_diagnostic:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) layout_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) layout_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

pub(super) struct Fixture {
    pub(super) foundation: ConeLirFoundation,
    pub(super) identities: RegistrationIdentitySurfaceV1,
    pub(super) semantics: StrongTypeDescriptorSemanticPlanSetV1,
    pub(super) digests: DigestFinalizationPlanV1,
}

impl Fixture {
    pub(super) fn new(options: Options) -> Self {
        let types = [type_artifacts(1), type_artifacts(2)];
        let interface = options
            .first_itable_interface
            .unwrap_or(types[1].exact_type);
        let first_itable =
            CborIdentityRecord::from_key(DispatchTableKey::itable(types[0].exact_type, interface))
                .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_layouts(
                types
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !(options.omit_first_layout && *index == 0))
                    .map(|(_, artifacts)| artifacts.layout.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_scans(
                types
                    .iter()
                    .map(|artifacts| artifacts.scan.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_dispatch_tables(
                types
                    .iter()
                    .map(|artifacts| artifacts.vtable.clone())
                    .chain(
                        options
                            .first_type_has_itable
                            .then_some(first_itable.clone()),
                    )
                    .collect(),
            )
            .unwrap();
        canonical
            .set_runtime_types(
                types
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !(options.omit_first_runtime_type && *index == 0))
                    .map(|(_, artifacts)| {
                        RuntimeTypeMappingRecord::new(artifacts.exact_type).unwrap()
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_plans(
                types
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            artifacts.descriptor_definition.clone(),
                            artifacts.layout_definition.clone(),
                            artifacts.registration_definition.clone(),
                        ]
                    })
                    .filter(|plan| {
                        !(options.omit_last_registration
                            && plan.id() == types[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        let mut atoms = types
            .iter()
            .flat_map(|artifacts| {
                [
                    artifacts.descriptor_primary.clone(),
                    artifacts.descriptor_diagnostic.clone(),
                    artifacts.layout_primary.clone(),
                    artifacts.registration_primary.clone(),
                ]
            })
            .filter(|atom| {
                !(options.omit_last_registration
                    && atom.key().plan() == types[1].registration_definition.id())
                    && !(options.omit_descriptor_diagnostic
                        && atom.id() == types[0].descriptor_diagnostic.id())
            })
            .collect::<Vec<_>>();
        if options.extra_descriptor_atom {
            atoms.push(
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    types[0].descriptor_definition.id(),
                    DefinitionAtomRole::RuntimeRecord,
                    DefinitionAtomSubkey::ExactType(types[0].exact_type),
                ))
                .unwrap(),
            );
        }
        if options.first_type_has_itable && !options.omit_itable_directory {
            atoms.push(
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    types[0].descriptor_definition.id(),
                    DefinitionAtomRole::RuntimeRecord,
                    DefinitionAtomSubkey::ExactType(types[0].exact_type),
                ))
                .unwrap(),
            );
        }
        canonical.set_definition_atoms(atoms).unwrap();
        let symbols = types
            .iter()
            .flat_map(|artifacts| {
                [
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::TypeDescriptor(artifacts.exact_type),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::Layout(artifacts.layout.id()),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::TypeRegistration(artifacts.exact_type),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                ]
            })
            .filter(|symbol| {
                !(options.omit_registration_symbol
                    && symbol.key() == PersistentSymbolKey::TypeRegistration(types[0].exact_type))
                    && !(options.omit_descriptor_symbol
                        && symbol.key() == PersistentSymbolKey::TypeDescriptor(types[0].exact_type))
                    && !(options.omit_layout_symbol
                        && symbol.key() == PersistentSymbolKey::Layout(types[0].layout.id()))
                    && !(options.omit_last_registration
                        && symbol.key()
                            == PersistentSymbolKey::TypeRegistration(types[1].exact_type))
            })
            .collect();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let digests = digest_plan(&foundation, &types, options);
        let identities =
            RegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let semantics = StrongTypeDescriptorSemanticPlanSetV1::from_artifact(
            ConeIdentity::SINGLE_FILE,
            crate::LirTargetProfile::DARWIN_AARCH64.wire_id(),
            types
                .iter()
                .enumerate()
                .map(|(index, artifacts)| {
                    let itables = if index == 0 && options.first_type_has_itable {
                        vec![StrongTypeItableSemanticPlanV1::from_artifact(
                            first_itable.id(),
                            StrongTypeDescriptorRefV1::Local(interface),
                            Vec::new(),
                        )]
                    } else {
                        Vec::new()
                    };
                    StrongTypeDescriptorSemanticPlanV1::from_artifact(
                        artifacts.exact_type,
                        format!("type-{}", artifacts.exact_type),
                        artifacts.layout.id(),
                        artifacts.scan.id(),
                        TypeInstanceShapeV1::fixed_object(
                            crate::LirTargetProfile::DARWIN_AARCH64,
                            16,
                            8,
                            RefScan::None,
                        )
                        .unwrap(),
                        TypeDescriptorInlineScanV1::Null,
                        None,
                        StrongTypeVtableSemanticPlanV1::from_artifact(
                            artifacts.vtable.id(),
                            Vec::new(),
                        ),
                        itables,
                        crate::TypeDescriptorRelations::Absent,
                    )
                })
                .collect(),
        );
        Self {
            foundation,
            identities,
            semantics,
            digests,
        }
    }

    pub(super) fn build(
        &self,
    ) -> Result<StrongTypeRegistrationPlanSetV1, StrongTypeRegistrationPlanBuildError> {
        StrongTypeRegistrationPlanSetV1::new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &self.foundation,
            &self.identities,
            &self.semantics,
            &self.digests,
        )
    }
}
