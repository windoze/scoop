use scoop_identity::{
    BindableEntity, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, DeclarationScope, DefinitionOwnerChain, Effect, ExportBindingKey, GcEffect,
    PackagePath, PersistentExportBindingId, PersistentFunctionId, SemanticOriginFingerprint,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::{call, file, fun, ident, sp, stmt};
use crate::{OrdinarySources, lower_ordinary};

#[test]
fn ordinary_dependency_calls_commit_one_reused_typed_hir_use() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "callable-provider",
        "run",
        SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 51);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![fun(
        "consumer",
        vec![stmt(call("run", Vec::new())), stmt(call("run", Vec::new()))],
    )]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "run"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 51),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
        .expect("a core-closed dependency function is executable in M23-5");

    assert_eq!(output.imported_dependencies().len(), 1);
    assert_eq!(
        output.output().export.imported_dependency_callables.len(),
        1
    );
    assert_eq!(output.output().local.imported_dependency_callables.len(), 1);
    assert_eq!(
        scoop_hir::dump(&output.output().export)
            .matches("ImportedDependencyCall #0")
            .count(),
        2
    );
}

#[test]
fn unsupported_dependency_candidate_falls_through_to_current_package() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "layout-provider",
        "run",
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 52);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![
        fun("run", Vec::new()),
        fun("consumer", vec![stmt(call("run", Vec::new()))]),
    ]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "run"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 52),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
        .expect("an unsupported exact candidate must not shadow a lower valid layer");

    assert!(output.imported_dependencies().is_empty());
    assert!(
        output
            .output()
            .export
            .imported_dependency_callables
            .is_empty()
    );
}

#[test]
fn unsupported_dependency_winner_reports_the_stable_layout_gate() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "layout-only-provider",
        "raw",
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 53);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![fun("consumer", vec![stmt(call("raw", Vec::new()))])]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "raw"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 53),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let diagnostics = match lower_ordinary(scoop_identity::RequestedConeKind::Library, &input) {
        Ok(_) => panic!("a dependency pointer result requires the M23-6 ABI capability"),
        Err(diagnostics) => diagnostics,
    };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED")
    }));
}

struct DependencyFunctionFixture {
    coordinate: ConeCoordinate,
    foundation: scoop_hir::CanonicalHirFoundation,
    interface: scoop_hir::CrossConeHirInterfaceSectionV1,
}

impl DependencyFunctionFixture {
    fn new(provider: &str, name: &str, result: SignatureTypeKey) -> Self {
        let coordinate = ConeCoordinate::new("test", provider, "1.0.0").unwrap();
        let origin = coordinate.identity().unwrap();
        let package = package_path(&["dependency", "api"]);
        let key = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                origin,
                package.clone(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(key.clone()).unwrap();
        let binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> =
            CborIdentityRecord::from_key(ExportBindingKey::new(
                origin,
                package,
                CanonicalIdentifier::new(name).unwrap(),
                BindingTarget::function(&key).unwrap(),
            ))
            .unwrap();
        let declaration = CallableTemplateOrigin::Function(function.id());
        let callable = scoop_hir::CallableInterfaceRecordV1::try_new(
            declaration,
            scoop_hir::PublicDeclarationOwnerV1::TopLevel,
            scoop_hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            scoop_hir::CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
            result,
            scoop_hir::CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                scoop_hir::CallableSafetyV1::Safe,
                GcEffect::Managed,
                scoop_hir::CallableImplementationV1::Scoop,
                scoop_hir::CallableOperatorRoleV1::None,
                scoop_hir::CallableInfixV1::Ordinary,
            )
            .unwrap(),
            scoop_hir::CallableModalityV1::Final,
            scoop_hir::PublicLookupAccessV1::DirectOnly,
        )
        .unwrap();
        let source = scoop_hir::CallableSourceInterfaceV1::try_new(
            declaration,
            scoop_hir::CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap();
        let public = scoop_hir::PublicExportBindingRecordV1::new(
            binding.id(),
            scoop_hir::ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::Function(function.id()),
            },
        );
        let mut foundation = scoop_hir::CanonicalHirFoundation::empty();
        foundation.set_functions(vec![function]).unwrap();
        foundation.set_export_bindings(vec![binding]).unwrap();
        let interface = scoop_hir::CrossConeHirInterfaceSectionV1::new(
            scoop_hir::CanonicalPublicExportBindingsV1::try_new(vec![public]).unwrap(),
            scoop_hir::CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap(),
            scoop_hir::CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap(),
            scoop_hir::CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
        );
        Self {
            coordinate,
            foundation,
            interface,
        }
    }
}

fn exact_import(segments: &[&str]) -> scoop_ast::ImportSyntax {
    let first = ident(segments[0]);
    let rest = segments[1..]
        .iter()
        .map(|segment| scoop_ast::QualifiedNameTailSyntax {
            dot_span: sp(),
            identifier: ident(segment),
        })
        .collect();
    scoop_ast::ImportSyntax::Exact {
        exposure: scoop_ast::ImportExposureSyntax::Local,
        selector: scoop_ast::QualifiedNameSyntax {
            first,
            rest,
            span: sp(),
        },
        alias: None,
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn package_path(segments: &[&str]) -> PackagePath {
    PackagePath::from_segments(
        segments
            .iter()
            .map(|segment| CanonicalIdentifier::new(segment).unwrap())
            .collect(),
    )
}

fn certificate(
    coordinate: &ConeCoordinate,
    fingerprint: u8,
) -> scoop_hir::ImportedProviderCertificate {
    scoop_hir::ImportedProviderCertificate::from_validated(
        coordinate.clone(),
        coordinate.identity().unwrap(),
        SemanticOriginFingerprint::new(
            [fingerprint; 32],
            [fingerprint.wrapping_add(1); 32],
            [fingerprint.wrapping_add(2); 32],
        ),
    )
}

fn empty_alias_expansions() -> scoop_hir::CanonicalTypeAliasExpansionsV1 {
    scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(
            &EmptyAliasAuthority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap()
}

fn empty_interface() -> scoop_hir::CrossConeHirInterfaceSectionV1 {
    scoop_hir::CrossConeHirInterfaceSectionV1::new(
        scoop_hir::CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        scoop_hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

struct EmptyAliasAuthority;

impl scoop_hir::TypeAliasClosureAuthority for EmptyAliasAuthority {
    fn external_type_alias(
        &self,
        _alias: scoop_identity::PersistentTypeAliasId,
    ) -> Option<&scoop_hir::TypeAliasInterfaceRecordV1> {
        None
    }

    fn is_type_alias_edge_authorized(
        &self,
        _source: scoop_identity::PersistentTypeAliasId,
        _target: scoop_identity::PersistentTypeAliasId,
    ) -> bool {
        false
    }
}
