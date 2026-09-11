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
        foundation.set_layouts(
            module
                .meta
                .layouts
                .iter()
                .map(|(_, layout)| layout.identity.layout_record().clone())
                .chain(
                    module
                        .meta
                        .arrays
                        .iter()
                        .map(|(_, array)| array.identity.layout_record().clone()),
                )
                .collect(),
        )?;
        foundation.set_scans(
            module
                .meta
                .layouts
                .iter()
                .map(|(_, layout)| layout.identity.scan_record().clone())
                .chain(
                    module
                        .meta
                        .arrays
                        .iter()
                        .map(|(_, array)| array.identity.scan_record().clone()),
                )
                .collect(),
        )?;
        foundation.project_layout_materializations(
            module
                .meta
                .layouts
                .iter()
                .map(|(_, layout)| &layout.identity)
                .chain(module.meta.arrays.iter().map(|(_, array)| &array.identity)),
        )?;
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
        foundation.project_global_identities(&module.globals)?;
        foundation.set_runtime_types(
            module
                .meta
                .type_descriptors
                .iter()
                .map(|(_, descriptor)| descriptor.runtime_type)
                .collect(),
        )?;
        Ok(foundation)
    }

    fn project_layout_materializations<'identity>(
        &mut self,
        identities: impl IntoIterator<Item = &'identity crate::LayoutIdentity>,
    ) -> Result<(), LirFoundationBuildError> {
        let mut groups = BTreeMap::new();
        let mut members = BTreeMap::new();
        for identity in identities {
            insert_projected_identity(
                &mut groups,
                identity.lir_odr_group_record(),
                LirFoundationTable::OdrGroup,
            )?;
            for member in identity.odr_member_records() {
                insert_projected_identity(
                    &mut members,
                    Some(member),
                    LirFoundationTable::OdrMember,
                )?;
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
                    crate::GlobalInit::StringConst { .. } | crate::GlobalInit::CString(_) => None,
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
                    crate::GlobalInit::CString(_) | crate::GlobalInit::Storage { .. } => None,
                })
                .collect(),
        )
    }

    fn from_functions(functions: &[Function]) -> Result<Self, LirFoundationBuildError> {
        let mut callable_bodies = Vec::with_capacity(functions.len());
        let safepoint_count = functions
            .iter()
            .map(|function| function.safepoints.len())
            .sum();
        let mut safepoint_sites = Vec::with_capacity(safepoint_count);
        let mut safepoints = Vec::with_capacity(safepoint_count);

        for (function_index, function) in functions.iter().enumerate() {
            let callable_body = function.callable_body.id();
            callable_bodies.push(function.callable_body.identity_record().clone());
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
        Ok(foundation)
    }
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
        DefinitionOwnerChain, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, NonEmptyVec,
        OdrMemberRole, PackagePath, PersistentExactTypeId, PersistentFunctionId, SafepointSiteRole,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey, StructuralDefinitionPath,
        StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    use super::*;
    use crate::{
        AbiReturn, BasicBlock, CallTargets, CallableBodyIdentity, CallingConvention, GcEffect,
        Global, GlobalInit, ImmortalObjectIdentity, LayoutIdentity, LirTargetProfile,
        MaterializationRoot, PointerKind, RefScan, SafepointIdentities, SafepointIdentity,
        SafepointSiteRef, ScoopAbiSignature, Terminator,
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
            .project_layout_materializations(&identities)
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
        let identity = ImmortalObjectIdentity::from_key(key.clone()).unwrap();
        let expected = identity.identity_record().clone();
        let mut globals = Arena::new();
        globals.alloc(Global {
            symbol: "scoop.str.0".to_string(),
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::StringConst {
                identity: identity.clone(),
                value: "text".to_string(),
            },
        });

        let mut foundation = CanonicalLirFoundation::empty();
        foundation.project_global_identities(&globals).unwrap();

        assert_eq!(foundation.immortal_objects, vec![expected]);
        assert!(foundation.static_storages.is_empty());
        assert_eq!(foundation.immortal_objects[0].key(), &key);
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
            symbol: "projection_test".to_string(),
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
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        PersistentFunctionId::from_source_declaration(&declaration).unwrap()
    }
}
