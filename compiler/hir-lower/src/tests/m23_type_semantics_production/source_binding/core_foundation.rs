use super::*;
use hir::CrossConeTypeSemanticsFoundationV1 as Production;
use scoop_identity::{ConeIdentity, DefinitionOriginSubject as Subject, SignatureTypeKey as Type};
mod defaults;
mod rejection;
mod snapshot;
mod support;
use support::{artifact, lower_minimal, lower_sysroot};

#[test]
fn core_source_foundation_replays_real_bootstrap_artifact() {
    let output = lower_minimal();
    replay(&output);
    assert_eq!(
        snapshot::render(&output),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/core-source-foundation.snap"
        ))
    );
}

#[test]
fn core_source_foundation_replays_full_sysroot_artifact() {
    let output = lower_sysroot();
    replay(&output);
    let origins = output.export.export_definition_origins.records();
    let declaration_origins = origins
        .iter()
        .filter(|record| !matches!(record.subject(), Subject::LocalBinding(_)))
        .map(|record| record.origin())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(declaration_origins.len(), 515);
    let all_origins = origins
        .iter()
        .map(|record| record.origin())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(all_origins.difference(&declaration_origins).count(), 37);
    for binding in output.export.local_binding_identities.iter() {
        assert_eq!(
            binding.record().key().source_role(),
            scoop_identity::LocalBindingRole::Declaration
        );
        assert_eq!(
            output
                .export
                .export_definition_origins
                .get(Subject::LocalBinding(binding.record().id()))
                .unwrap()
                .origin(),
            binding.origin()
        );
    }
    assert_eq!(
        snapshot::render(&output),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/core-source-sysroot.snap"
        ))
    );
}

fn replay(output: &hir::Output) {
    let source = Production::from_hir(output, &mut meter()).unwrap();
    let transcript = source.source_transcript(&mut meter()).unwrap();
    assert!(!transcript.entries().source_roots.values().is_empty());
    assert!(!transcript.entries().representations.records().is_empty());
    assert!(!transcript.entries().definition_sources.sources().is_empty());
    assert!(transcript.entries().dependency_facts.records().is_empty());
    let (foundation, mut identities) = artifact(output);
    let bytes = encode(&transcript).unwrap();
    let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded.resolve(&mut identities, &mut meter()).unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    let bound = restored
        .bind_to_foundation(&foundation, &identities, &mut meter())
        .unwrap();
    for owner in source.source_roots() {
        assert_eq!(
            bound.nominal_key(*owner).unwrap(),
            source.nominal_declaration_key(*owner).unwrap()
        );
    }
    assert!(source.source_fact_shapes().any(|(id, _)| matches!(
        bound.exact_type_key(id).unwrap(),
        scoop_identity::ExactTypeKey::NominalApplication { .. }
    )));
}

#[test]
fn core_source_foundation_binds_real_boolean_and_pointer_access_domains() {
    let output = lower_minimal();
    let source = Production::from_hir(&output, &mut meter())
        .unwrap()
        .source_transcript(&mut meter())
        .unwrap();
    let (foundation, identities) = artifact(&output);
    let required = source
        .entries()
        .source_roots
        .values()
        .iter()
        .map(|id| match *id {
            hir::SourceNominalId::Concrete(id) => Subject::Type(id),
            hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
        })
        .collect();
    let access = hir::CanonicalDefaultSourceAccessDeclarationsV1::from_export_hir(
        &output.export,
        &required,
        &mut meter(),
    )
    .unwrap();
    let bound = source
        .bind_to_foundation(&foundation, &identities, &mut meter())
        .unwrap();
    let access = bound
        .bind_default_access_declarations(&access, &required, &mut meter())
        .unwrap();
    let imported = support::import(&foundation, &identities);
    let interface = hir::CompilerProtocolDefinitionsV1::from_export(&output.export).unwrap();
    let inputs = imported.import_core_inputs(&interface).unwrap();
    let core = inputs.protocols().fundamental_types();
    let domains = hir::DefaultSourceTypeDomainsV1::new(&access, &[], core, &mut meter()).unwrap();
    let scope = hir::SignatureBinderScopeV1::for_declaration(0, None);
    let unit = Type::Nominal(core.unit().persistent());
    for ty in [
        Type::Nominal(core.boolean().persistent()),
        Type::RawPointer(Box::new(unit.clone())),
        Type::NativeFunctionPointer {
            calling_convention: scoop_identity::CallingConvention::C,
            parameters: vec![Type::Nominal(core.boolean().persistent())],
            result: Box::new(unit),
        },
    ] {
        assert_eq!(
            domains
                .type_source_domain(&ty, &scope, &mut meter())
                .unwrap(),
            hir::DefaultSourceAccessDomainV1::universal()
        );
    }
}

#[test]
fn ordinary_and_core_sources_share_the_same_foundation_projection() {
    super::super::source_dispatch::with_hir_source("public struct Value()", |output, _| {
        let common = Production::from_hir(output.output(), &mut meter()).unwrap();
        let ordinary = Production::from_dependency_hir(output, &mut meter()).unwrap();
        assert_eq!(
            encode(&common.source_transcript(&mut meter()).unwrap()).unwrap(),
            encode(&ordinary.source_transcript(&mut meter()).unwrap()).unwrap()
        );
    });
}
