use std::collections::BTreeSet;

use scoop_identity::{
    BindingNamespace, BindingTarget, ConeCoordinate, ConeIdentity, ExportBindingKey,
    NominalDeclarationOwner,
};

use super::super::super::m23_ordinary_core_only::support::{
    TrustedCoreFixture, parsed_ordinary_at,
};
use crate::{OrdinaryCoreOnlySources, lower_ordinary_core_only};

pub(crate) fn project_dependency(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    default_core_types: &[&str],
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    project_dependency_with_core_roles(core, coordinate, source, default_core_types, true)
}

pub(crate) fn project_dependency_without_default_roles(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    core_types: &[&str],
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    project_dependency_with_core_roles(core, coordinate, source, core_types, false)
}

fn project_dependency_with_core_roles(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    core_types: &[&str],
    include_default_role: bool,
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    let parsed = parsed_ordinary_at(coordinate, source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let witnesses = core_types
        .iter()
        .flat_map(|name| core_type_witnesses(&core_inputs, name, include_default_role))
        .collect::<Vec<_>>();
    let input = OrdinaryCoreOnlySources::try_new(&parsed, core_inputs).unwrap();
    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Library, &input)
        .expect("the dependency provider must lower before interface projection");
    let current_nominals = current_nominal_targets(output.output().export.module());
    let foundation = scoop_hir::CanonicalHirFoundation::from_ordinary_output(&output).unwrap();
    let mut authority = ProviderProjectionAuthority {
        provider: coordinate.identity().unwrap(),
        current_nominals,
    };
    let interface = scoop_hir::CrossConeHirInterfaceSectionV1::from_ordinary_hir(
        &output,
        &witnesses,
        &mut authority,
    )
    .unwrap();
    (foundation, interface)
}

fn core_type_witnesses(
    core: &scoop_hir::ImportedCoreInputs<'_>,
    name: &str,
    include_default_role: bool,
) -> Vec<scoop_hir::ExternalHirBindingWitnessUse> {
    let binding = core
        .prelude()
        .candidates(BindingNamespace::Type, name)
        .next()
        .unwrap_or_else(|| panic!("the trusted core fixture exports {name}"));
    let scoop_hir::ImportedCorePreludeTarget::Type(target) = binding.target() else {
        panic!("the {name} prelude binding must target a type")
    };
    let scoop_hir::CoreTypeDefinitionV1::Type(declaration) = target.definition() else {
        panic!("the {name} prelude binding must target a concrete nominal")
    };
    let route = scoop_hir::ReexportRouteV1::try_new(
        ConeIdentity::CORE,
        vec![scoop_hir::ReexportRouteHopV1::new(
            ConeIdentity::CORE,
            binding.identity().persistent(),
        )],
    )
    .unwrap();
    let target = scoop_hir::ExternalHirTargetV1::Nominal(
        scoop_identity::NominalDeclarationOwner::Concrete(declaration),
    );
    let mut witnesses = Vec::with_capacity(1 + usize::from(include_default_role));
    if include_default_role {
        witnesses.push(scoop_hir::ExternalHirBindingWitnessUse::new(
            target,
            scoop_hir::ExternalHirBindingWitnessRole::DefaultDependency,
            scoop_hir::DependencyBindingWitnessV1::new(route.clone()),
        ));
    }
    witnesses.push(scoop_hir::ExternalHirBindingWitnessUse::new(
        target,
        scoop_hir::ExternalHirBindingWitnessRole::ConcreteSelectedUse,
        scoop_hir::DependencyBindingWitnessV1::new(route),
    ));
    witnesses
}

struct ProviderProjectionAuthority {
    provider: ConeIdentity,
    current_nominals: BTreeSet<scoop_hir::ExternalHirTargetV1>,
}

fn current_nominal_targets(module: &scoop_hir::Module) -> BTreeSet<scoop_hir::ExternalHirTargetV1> {
    let mut targets = BTreeSet::new();
    let mut insert = |identity: &scoop_hir::HirNominalIdentity| {
        let owner = match (identity.concrete_type_id(), identity.generic_type_id()) {
            (Some(id), None) => NominalDeclarationOwner::Concrete(id),
            (None, Some(id)) => NominalDeclarationOwner::GenericTemplate(id),
            _ => unreachable!("a source nominal has exactly one persistent identity kind"),
        };
        targets.insert(scoop_hir::ExternalHirTargetV1::Nominal(owner));
    };
    for (id, _) in module.structs.iter() {
        insert(&module.nominal_identities[id]);
    }
    for (id, _) in module.enums.iter() {
        insert(&module.nominal_identities[id]);
    }
    for (id, _) in module.classes.iter() {
        insert(&module.nominal_identities[id]);
    }
    for (id, _) in module.interfaces.iter() {
        insert(&module.nominal_identities[id]);
    }
    for (id, _) in module.objects.iter() {
        insert(&module.nominal_identities[id]);
    }
    targets
}

impl scoop_hir::PublicExportBindingClosureAuthority for ProviderProjectionAuthority {
    fn closure_node_count(&self) -> usize {
        1
    }

    fn is_direct_dependency(&self, _provider: ConeIdentity) -> bool {
        false
    }

    fn binding_key(
        &self,
        _binding: scoop_identity::PersistentExportBindingId,
    ) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(
        &self,
        _exporter: ConeIdentity,
    ) -> Option<&scoop_hir::CanonicalPublicExportBindingsV1> {
        None
    }
}

impl scoop_hir::ExternalHirReferenceSemanticAuthority<&'static str>
    for ProviderProjectionAuthority
{
    fn current_cone(&self) -> ConeIdentity {
        self.provider
    }

    fn external_hir_target_origin(
        &mut self,
        target: scoop_hir::ExternalHirTargetV1,
    ) -> Result<ConeIdentity, &'static str> {
        if self.current_nominals.contains(&target) {
            return Ok(self.provider);
        }
        Ok(match target {
            scoop_hir::ExternalHirTargetV1::Nominal(_) => ConeIdentity::CORE,
            _ => self.provider,
        })
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: scoop_hir::ExternalHirTargetV1,
    ) -> Result<BindingTarget, &'static str> {
        Err("the provider fixture has no external binding witnesses")
    }
}
