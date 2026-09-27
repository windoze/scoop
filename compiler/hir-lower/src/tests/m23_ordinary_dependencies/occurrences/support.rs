use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, ExportBindingKey, PackagePath, PersistentExportBindingId,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text;

pub(super) fn lower(source: &str) -> hir::DependencyHirOutput {
    with_output(source, |output, _| output)
}

pub(super) fn with_output<T>(
    source: &str,
    inspect: impl FnOnce(hir::DependencyHirOutput, &hir::ImportedSemanticWorld<'_>) -> T,
) -> T {
    let mut core = trusted_core();
    let hir::ImportedTarget::Type(boolean) = core.type_binding("Boolean").target() else {
        panic!("Boolean is nominal")
    };
    let provider = DependencyFunctionFixture::new(
        "occurrence-provider",
        "run",
        SignatureTypeKey::Nominal(boolean.persistent()),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 91);
    let (facade, external) = facade(&provider);
    let facade_foundation = core.import_dependency_foundation_with_functions(
        &facade.coordinate,
        &facade.foundation,
        92,
        &[external],
    );
    let ordinary = parsed_ordinary_text(source);
    let aliases = empty_alias_expansions();
    let world = hir::ImportedSemanticWorld::from_dependencies(
        ordinary.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &provider_foundation,
                interface: &provider.interface,
                alias_expansions: &aliases,
            },
            hir::ImportedProviderInput {
                foundation: &facade_foundation,
                interface: &facade.interface,
                alias_expansions: &aliases,
            },
        ],
        vec![],
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    inspect(output, &world)
}

fn package(parts: &[&str]) -> PackagePath {
    PackagePath::from_segments(
        parts
            .iter()
            .map(|part| CanonicalIdentifier::new(part).unwrap())
            .collect(),
    )
}

fn facade(
    provider: &DependencyFunctionFixture,
) -> (
    DependencyFunctionFixture,
    CborIdentityRecord<scoop_identity::PersistentFunctionId, SourceDeclarationKey>,
) {
    let coordinate = ConeCoordinate::new("test", "occurrence-facade", "1.0.0").unwrap();
    let terminal = provider.coordinate.identity().unwrap();
    let source_key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            terminal,
            package(&["dependency", "api"]),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        vec![],
    );
    let binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> =
        CborIdentityRecord::from_key(ExportBindingKey::new(
            coordinate.identity().unwrap(),
            package(&["facade", "api"]),
            CanonicalIdentifier::new("run").unwrap(),
            scoop_identity::BindingTarget::function(&source_key).unwrap(),
        ))
        .unwrap();
    let route = hir::ReexportRouteV1::try_new(
        terminal,
        vec![hir::ReexportRouteHopV1::new(
            terminal,
            provider.interface.public_bindings().records()[0].binding(),
        )],
    )
    .unwrap();
    let mut foundation = hir::CanonicalHirFoundation::empty();
    foundation
        .set_export_bindings(vec![binding.clone()])
        .unwrap();
    let interface = hir::CrossConeHirInterfaceSectionV1::new(
        hir::CanonicalPublicExportBindingsV1::try_new(vec![hir::PublicExportBindingRecordV1::new(
            binding.id(),
            hir::ExportBindingSourceV1::Reexport {
                routes: hir::CanonicalReexportRoutesV1::try_new(vec![route]).unwrap(),
            },
        )])
        .unwrap(),
        hir::CanonicalNominalInterfacesV1::try_new(vec![]).unwrap(),
        hir::CanonicalCallableInterfacesV1::try_new(vec![]).unwrap(),
        hir::CanonicalPropertyInterfacesV1::try_new(vec![]).unwrap(),
        hir::CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
        hir::CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        hir::CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        hir::CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
        hir::CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
        hir::CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
        Default::default(),
    );
    let external = CborIdentityRecord::from_key(source_key).unwrap();
    (
        DependencyFunctionFixture {
            coordinate,
            foundation,
            interface,
        },
        external,
    )
}

pub(super) fn root_name(
    module: &hir::concrete::Module,
    root: scoop_identity::CallableMaterialization,
) -> String {
    if let Some((_, function)) = module
        .functions
        .iter()
        .find(|(_, function)| function.materialization == root)
    {
        return function.name.clone();
    }
    if let Some((_, constructor)) = module
        .class_constructors
        .iter()
        .find(|(_, constructor)| constructor.materialization == root)
    {
        return format!(
            "class {} constructor {}",
            module.classes[constructor.class].name, constructor.source_discriminator
        );
    }
    let (_, constructor) = module
        .struct_constructors
        .iter()
        .find(|(_, constructor)| constructor.materialization == root)
        .unwrap();
    format!(
        "struct {} constructor {}",
        module.structs[constructor.structure].name, constructor.source_discriminator
    )
}
