use std::path::PathBuf;

mod world;

use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, NormalizedSourcePath, PendingIdentityValidation,
    SemanticIdentitySession, SemanticOriginFingerprint, SourceIdentity,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::super::{
    complete_core_file, fun_expr, int_lit, make_core_public, test_source_identity, ty_named,
};
use crate::{CoreBootstrapSources, lower_core_bootstrap};

pub(crate) struct TrustedCoreFixture {
    pub(crate) foundation: scoop_hir::ImportedHirFoundation,
    pub(crate) source_foundation: scoop_hir::OdrFreeHirFoundation,
    general_interface: scoop_hir::CrossConeHirInterfaceSectionV1,
    aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    pub(crate) interface: scoop_hir::CoreHirInterfaceV1,
    mir_foundation: scoop_mir::ImportedMirFoundation,
    mir_production: scoop_mir::CoreBootstrapBridgeSectionV1,
    session: SemanticIdentitySession,
}

impl TrustedCoreFixture {
    pub(crate) fn project_initialization_cycle_to_mir(
        &self,
    ) -> scoop_mir::SelectedDependencyMirCallableV1 {
        let scoop_hir::CoreProtocolCallableDefinitionV1::Function(definition) = self
            .interface
            .compiler_protocols()
            .initialization_cycle_thrower()
            .definition()
        else {
            panic!("the fixture cycle thrower is a source function")
        };
        let signature = scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![self.interface.string_capability().exact_type()],
            scoop_mir::core_unit_exact_type(),
        );
        self.mir_foundation
            .project_initialization_cycle_thrower(&self.mir_production, definition, signature)
            .unwrap()
    }

    pub(crate) fn import_dependency_foundation(
        &mut self,
        coordinate: &scoop_identity::ConeCoordinate,
        foundation: &scoop_hir::CanonicalHirFoundation,
        fingerprint: u8,
    ) -> scoop_hir::ImportedHirFoundation {
        let decoded: scoop_hir::DecodedHirFoundation =
            decode_canonical(&encode(foundation).unwrap(), DecodeLimits::default()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(coordinate.identity().unwrap())
            .unwrap();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        decoded.register_identities(&mut pending).unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        let identities = pending.finish().unwrap();
        let imported = self
            .session
            .import(
                coordinate.identity().unwrap(),
                SemanticOriginFingerprint::new(
                    [fingerprint; 32],
                    [fingerprint.wrapping_add(1); 32],
                    [fingerprint.wrapping_add(2); 32],
                ),
                &identities,
            )
            .unwrap();
        let (hir, _, _) = imported.into_parts();
        scoop_hir::ImportedHirFoundation::from_odr_free(
            scoop_hir::OdrFreeHirFoundation::try_new(foundation.clone()).unwrap(),
            hir,
        )
    }
}

pub(crate) fn trusted_core() -> TrustedCoreFixture {
    trusted_core_from_source(complete_core_file(), "", None)
}

pub(super) fn trusted_core_with_answer() -> TrustedCoreFixture {
    let mut source = complete_core_file();
    source.declarations.push(fun_expr(
        "coreAnswer",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        int_lit(42),
    ));
    make_core_public(&mut source);
    trusted_core_from_source(source, "", Some("coreAnswer"))
}

pub(super) fn trusted_core_from_source(
    source: scoop_ast::SourceFile,
    source_text: &str,
    strong_callable: Option<&str>,
) -> TrustedCoreFixture {
    let parsed = parsed_sources(
        super::super::core_source_identity("src/core.scoop"),
        source,
        "<core>",
        source_text,
    );
    let input = CoreBootstrapSources::try_new(&parsed).unwrap();
    let output = lower_core_bootstrap(&input).unwrap();
    let interface = scoop_hir::CoreHirInterfaceV1::from_core_export(&output.export).unwrap();
    let strong_definition = strong_callable.map(|name| {
        let function = output
            .export
            .top_level
            .iter()
            .copied()
            .find(|function| output.export.functions[*function].name == name)
            .unwrap();
        let scoop_hir::HirFunctionIdentity::Source(scoop_hir::HirSourceFunctionIdentity::Plain(
            identity,
        )) = &output.export.function_identities[function]
        else {
            panic!("test strong callable has a plain source identity")
        };
        identity.id()
    });
    let mut canonical = scoop_hir::CanonicalHirFoundation::from_modules(
        &output.export,
        &output.local,
        &output.native_boundary_types,
    )
    .unwrap();
    let general_interface = world::project_interface(&output, &mut canonical);
    let aliases = crate::tests::m23_ordinary_dependencies::support::alias_expansions(
        general_interface.type_aliases(),
    );
    let decoded: scoop_hir::DecodedHirFoundation =
        decode_canonical(&encode(&canonical).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let imported = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap();
    let (hir, mir, _) = imported.into_parts();
    let source_foundation = scoop_hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
    let foundation =
        scoop_hir::ImportedHirFoundation::from_odr_free(source_foundation.clone(), hir);
    let classifier = scoop_hir::CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(
        general_interface.nominal_interfaces().records(),
    )
    .unwrap();
    let strong_mir = strong_definition
        .map(|definition| {
            let record = general_interface
                .callable_interfaces()
                .get(scoop_identity::CallableTemplateOrigin::Function(definition))
                .unwrap();
            let callable = classifier.classify_callable(record).unwrap().unwrap();
            (definition, callable.signature().clone())
        })
        .into_iter()
        .collect::<Vec<_>>();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) = interface
        .compiler_protocols()
        .initialization_cycle_thrower()
        .definition()
    else {
        panic!("test initialization-cycle protocol is a source function")
    };
    let cycle_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![interface.string_capability().exact_type()],
        scoop_mir::core_unit_exact_type(),
    );
    let mut mir_canonical = scoop_mir::CanonicalMirFoundation::empty();
    mir_canonical
        .set_callable_signatures(
            strong_mir
                .iter()
                .map(|(definition, signature)| {
                    scoop_mir::CallableSignatureRecord::new(
                        scoop_mir::CallableSignatureSubject::Strong(
                            scoop_identity::CallableOwner::Function(*definition),
                        ),
                        signature.clone(),
                    )
                })
                .chain(std::iter::once(scoop_mir::CallableSignatureRecord::new(
                    scoop_mir::CallableSignatureSubject::Strong(
                        scoop_identity::CallableOwner::Function(cycle_definition),
                    ),
                    cycle_signature.clone(),
                )))
                .collect(),
        )
        .unwrap();
    let mir_foundation = scoop_mir::ImportedMirFoundation::from_odr_free(
        scoop_mir::OdrFreeMirFoundation::try_new(mir_canonical).unwrap(),
        mir,
    );
    let mir_production = scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        scoop_mir::EntryMirBridgeBranchV1::Library,
        scoop_mir::StrongCallableBridgeSurfaceV1::try_new(
            strong_mir
                .iter()
                .map(|(definition, signature)| {
                    scoop_mir::StrongCallableBridgeV1::new(
                        scoop_identity::CallableOwner::Function(*definition),
                        signature.clone(),
                    )
                })
                .chain(std::iter::once(scoop_mir::StrongCallableBridgeV1::new(
                    scoop_identity::CallableOwner::Function(cycle_definition),
                    cycle_signature,
                )))
                .collect(),
        )
        .unwrap()
        .with_initialization_cycle(cycle_definition)
        .unwrap(),
    )
    .unwrap();
    TrustedCoreFixture {
        general_interface,
        aliases,
        foundation,
        source_foundation,
        interface,
        mir_foundation,
        mir_production,
        session,
    }
}

pub(crate) fn parsed_core(source: scoop_ast::SourceFile) -> CurrentConeParsedSources {
    let identity = super::super::core_source_identity("src/core.scoop");
    parsed_sources(identity, source, "<core>", "")
}

pub(crate) fn parsed_ordinary(source: scoop_ast::SourceFile) -> CurrentConeParsedSources {
    parsed_sources(test_source_identity("src/main.scoop"), source, "<main>", "")
}

pub(crate) fn parsed_ordinary_at(
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
) -> CurrentConeParsedSources {
    let identity = SourceIdentity::new(
        coordinate.identity().unwrap(),
        NormalizedSourcePath::new("src/main.scoop").unwrap(),
    )
    .unwrap();
    parsed_sources(identity, source, "<dependency-main>", "")
}

fn parsed_sources(
    identity: SourceIdentity,
    source: scoop_ast::SourceFile,
    display: &str,
    source_text: &str,
) -> CurrentConeParsedSources {
    CurrentConeParsedSources::try_new(
        AllParsedSources::try_new(NonEmptyVec::new(
            IdentifiedParsedSource::new(identity.clone(), source),
            Vec::new(),
        ))
        .unwrap(),
        NonEmptyVec::new(
            CurrentSourceText::new(identity.clone(), source_text.to_owned()),
            Vec::new(),
        ),
        NonEmptyVec::new(
            CurrentSourceDiagnosticContext::new(identity, PathBuf::from(display)),
            Vec::new(),
        ),
    )
    .unwrap()
}

pub(super) fn parsed_ordinary_text(source: &str) -> CurrentConeParsedSources {
    parsed_sources(
        test_source_identity("src/main.scoop"),
        scoop_parser::parse(source).unwrap(),
        "<main>",
        source,
    )
}
