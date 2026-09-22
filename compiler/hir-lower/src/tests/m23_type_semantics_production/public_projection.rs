use super::*;

pub(super) fn public_interface(
    output: &hir::DependencyHirOutput,
) -> hir::CrossConeHirInterfaceSectionV1 {
    let export = output.output().export.module();
    let mut authority = PublicProjectionAuthority {
        current: export.cone,
        local_targets: local_targets(export),
    };
    hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(output, &[], &mut authority).unwrap()
}

struct PublicProjectionAuthority {
    current: ConeIdentity,
    local_targets: BTreeSet<hir::ExternalHirTargetV1>,
}

impl hir::PublicExportBindingClosureAuthority for PublicProjectionAuthority {
    fn closure_node_count(&self) -> usize {
        1
    }

    fn is_direct_dependency(&self, _provider: ConeIdentity) -> bool {
        false
    }

    fn binding_key(&self, _binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(
        &self,
        _exporter: ConeIdentity,
    ) -> Option<&hir::CanonicalPublicExportBindingsV1> {
        None
    }
}

impl hir::ExternalHirReferenceSemanticAuthority<&'static str> for PublicProjectionAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: hir::ExternalHirTargetV1,
    ) -> Result<ConeIdentity, &'static str> {
        Ok(if self.local_targets.contains(&target) {
            self.current
        } else {
            ConeIdentity::CORE
        })
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: hir::ExternalHirTargetV1,
    ) -> Result<BindingTarget, &'static str> {
        Err("this core-only fixture has no external binding witness")
    }
}

fn local_targets(export: &hir::ExportHir) -> BTreeSet<hir::ExternalHirTargetV1> {
    let mut targets = BTreeSet::new();
    let mut insert = |identity: &hir::HirNominalIdentity| {
        let Some(source) = identity.source() else {
            return;
        };
        let owner = match source {
            hir::HirSourceNominalIdentity::Concrete(record) => {
                NominalDeclarationOwner::Concrete(record.id())
            }
            hir::HirSourceNominalIdentity::Generic(record) => {
                NominalDeclarationOwner::GenericTemplate(record.id())
            }
        };
        targets.insert(hir::ExternalHirTargetV1::Nominal(owner));
    };
    for (id, _) in export.structs.iter() {
        insert(&export.nominal_identities[id]);
    }
    for (id, _) in export.enums.iter() {
        insert(&export.nominal_identities[id]);
    }
    for (id, _) in export.classes.iter() {
        insert(&export.nominal_identities[id]);
    }
    for (id, _) in export.interfaces.iter() {
        insert(&export.nominal_identities[id]);
    }
    for (id, _) in export.objects.iter() {
        insert(&export.nominal_identities[id]);
    }
    for (id, _) in export.struct_constructors.iter() {
        targets.insert(hir::ExternalHirTargetV1::Callable(
            scoop_identity::CallableTemplateOrigin::Constructor(
                export.constructor_identities[id].id(),
            ),
        ));
    }
    for (id, _) in export.class_constructors.iter() {
        if let Some(record) = export.constructor_identities[id].source_record() {
            targets.insert(hir::ExternalHirTargetV1::Callable(
                scoop_identity::CallableTemplateOrigin::Constructor(record.id()),
            ));
        }
    }
    for (id, _) in export.functions.iter() {
        let hir::HirFunctionIdentity::Source(source) = &export.function_identities[id] else {
            continue;
        };
        let declaration = match source {
            hir::HirSourceFunctionIdentity::Plain(record) => {
                scoop_identity::CallableTemplateOrigin::Function(record.id())
            }
            hir::HirSourceFunctionIdentity::Generic(record) => {
                scoop_identity::CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        targets.insert(hir::ExternalHirTargetV1::Callable(declaration));
    }
    targets
}
