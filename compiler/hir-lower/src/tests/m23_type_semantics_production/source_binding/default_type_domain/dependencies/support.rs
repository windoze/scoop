use super::*;
use crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture;
use scoop_identity::{
    ConeCoordinate, ConeIdentity, NormalizedSourcePath, PendingIdentityValidation, SourceIdentity,
};

pub(in crate::tests::m23_type_semantics_production::source_binding) struct Artifact {
    pub coordinate: ConeCoordinate,
    pub source: hir::TypeFoundationSourceAuthorityV1,
    pub foundation: hir::OdrFreeHirFoundation,
    pub table: Table,
    pub required: BTreeSet<Subject>,
    pub ty: Type,
    pub defaults: hir::CanonicalDefaultSourceTemplatesV1,
    pub contracts: super::super::super::nominal_parameters::support::Sources,
}
pub(in crate::tests::m23_type_semantics_production::source_binding) fn artifacts(
    core: &TrustedCoreFixture,
) -> ([Artifact; 3], ValidatedIdentityGraph) {
    let mut artifacts =
        ["type-current", "type-first", "type-second"].map(|name| artifact(core, name));
    let decode = |value: &hir::CanonicalHirFoundation| -> hir::DecodedHirFoundation {
        decode_canonical(&encode(value).unwrap()).unwrap()
    };
    let core_decoded = decode(core.source_foundation.as_canonical());
    let decoded = artifacts
        .each_ref()
        .map(|a| decode(a.foundation.as_canonical()));
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    core_decoded.register_identities(&mut pending).unwrap();
    core_decoded.resolve_identities(&mut pending).unwrap();
    let core_graph = pending.finish().unwrap();
    let mut combined = PendingIdentityValidation::new();
    combined
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    for (artifact, decoded) in artifacts.iter_mut().zip(decoded) {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(artifact.coordinate.identity().unwrap())
            .unwrap();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        decoded.register_identities(&mut pending).unwrap();
        pending
            .register_external_graph_authorities(&core_graph)
            .unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        artifact.foundation = hir::OdrFreeHirFoundation::from_validated(
            decoded
                .validate_with_dependency_sources(&artifact.coordinate, &mut identities)
                .unwrap(),
        )
        .unwrap();
        combined
            .register_external_graph_authorities(&identities)
            .unwrap();
    }
    let mut identities = combined.finish().unwrap();
    for artifact in &mut artifacts {
        let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&encode(&artifact.source).unwrap()).unwrap();
        artifact.source = decoded.resolve(&mut identities).unwrap();
        let decoded: hir::DecodedCanonicalDefaultSourceAccessDeclarationsV1 =
            decode_canonical(&encode(&artifact.table).unwrap()).unwrap();
        artifact.table = decoded.resolve(&mut identities).unwrap();
    }
    (artifacts, identities)
}
fn artifact(core: &TrustedCoreFixture, name: &str) -> Artifact {
    let coordinate = ConeCoordinate::new("test", name, "0.0.0").unwrap();
    let identity = SourceIdentity::new(
        coordinate.identity().unwrap(),
        NormalizedSourcePath::new("src/provider.scoop").unwrap(),
    )
    .unwrap();
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/type-domain-provider.scoop"
    ));
    let parsed = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(identity.clone(), scoop_parser::parse(text).unwrap()),
            vec![],
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), text.to_owned()),
            vec![],
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(
                identity,
                std::path::PathBuf::from("src/provider.scoop"),
            ),
            vec![],
        ),
    )
    .unwrap();
    let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(&parsed, inputs, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let export = output.output().export.module();
    let pick = export
        .functions
        .iter()
        .find(|(_, f)| f.name == "pick")
        .unwrap()
        .0;
    let template = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        &output,
        hir::ExportParameterOwner::Function(pick),
        1,
    )
    .unwrap();
    let ty = template.result().clone();
    let Type::Nominal(id) = ty else {
        panic!("private source nominal")
    };
    let mut required = BTreeSet::from([Subject::Type(id)]);
    let source = hir::CrossConeTypeSemanticsFoundationV1::from_dependency_hir(&output)
        .unwrap()
        .source_transcript()
        .unwrap();
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(&output).unwrap(),
    )
    .unwrap();
    let defaults = hir::NominalDefaultSourceProductionV1::from_dependency_hir(&output)
        .unwrap()
        .templates()
        .clone();
    let mut fixture = Fixture {
        source: source.clone(),
        foundation: foundation.clone(),
        identities: identity_closure(&output),
    };
    let contracts = super::super::super::nominal_parameters::support::Sources::from_output(
        &output,
        &mut fixture,
    );
    let bound = source
        .bind_to_foundation(&foundation, &fixture.identities)
        .unwrap();
    required
        .extend(super::super::super::default_value_domain::support::required(&bound, &defaults));
    required
        .extend(super::super::super::default_callable_domain::support::required(&bound, &defaults));
    let table = Table::from_export_hir(&output.output().export, &required).unwrap();
    Artifact {
        coordinate,
        source,
        foundation,
        table,
        required,
        ty,
        defaults,
        contracts,
    }
}
