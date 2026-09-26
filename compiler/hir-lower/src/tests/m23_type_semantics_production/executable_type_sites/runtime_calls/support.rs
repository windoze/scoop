use super::*;
use crate::tests::m23_ordinary_core_only::support::{
    TrustedCoreFixture, parsed_ordinary_text, trusted_core_from_source,
};
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};

pub(super) fn runtime_fixture(case: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tests/fixtures/m23-executable-type-sites/{case}.scoop"
        )),
    )
    .unwrap()
}

pub(super) fn with_fixture(
    case: &str,
    exception: &str,
    run: impl FnOnce(&hir::DependencyHirOutput, &TrustedCoreFixture),
) {
    let mut source = complete_core_file();
    let text = runtime_fixture(exception);
    let replacement = scoop_parser::parse(&text).unwrap().declarations.remove(0);
    let declaration = source.declarations.iter_mut().find(|declaration| {
        matches!(declaration, ast::Decl::Class(class) if class.name.text == "ClassCastException")
    }).unwrap();
    *declaration = replacement;
    make_core_public(&mut source);
    let core = trusted_core_from_source(source, &text);
    let ordinary = parsed_ordinary_text(&runtime_fixture(case));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    run(&output, &core);
}

pub(super) fn identities(
    output: &hir::DependencyHirOutput,
    core: &TrustedCoreFixture,
) -> ValidatedIdentityGraph {
    let decode = |foundation: &hir::CanonicalHirFoundation| {
        decode_canonical::<hir::DecodedHirFoundation>(&encode(foundation).unwrap()).unwrap()
    };
    let dependency = decode(core.source_foundation.as_canonical());
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    dependency.register_identities(&mut pending).unwrap();
    dependency.resolve_identities(&mut pending).unwrap();
    let dependency = pending.finish().unwrap();
    let current = decode(&hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap());
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(output.output().export.cone)
        .unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    current.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(&dependency)
        .unwrap();
    current.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

pub(super) fn with_metadata(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    core: &TrustedCoreFixture,
    run: impl FnOnce(hir::SharedTypeMetadataV1<'_>, hir::SharedTypeMetadataV1<'_>),
) {
    let identities = identities(output, core);
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap(),
    )
    .unwrap();
    run(
        hir::SharedTypeMetadataV1 {
            provider: output.output().export.cone,
            identities: &identities,
            foundation: &foundation,
            public,
        },
        hir::SharedTypeMetadataV1 {
            provider: ConeIdentity::CORE,
            identities: &identities,
            foundation: &core.source_foundation,
            public: core.general_interface(),
        },
    );
}

pub(super) fn calls(
    public: &hir::CrossConeHirInterfaceSectionV1,
) -> impl Iterator<Item = (&hir::ExternalHirReferenceV1, &hir::HirDependencyCallSiteV1)> {
    public
        .external_references()
        .records()
        .iter()
        .flat_map(|reference| {
            reference
                .call_sites()
                .records()
                .iter()
                .filter_map(move |site| {
                    matches!(site.reason(), HirDependencyCallReasonV1::CastFailure { .. })
                        .then_some((reference, site))
                })
        })
}
