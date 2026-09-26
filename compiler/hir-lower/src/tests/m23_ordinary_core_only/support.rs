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
use scoop_wire::{decode_canonical, encode};

use super::super::{
    complete_core_file, fun_expr, int_lit, make_core_public, test_source_identity, ty_named,
};
use crate::lower_core_bootstrap;

pub(crate) struct TrustedCoreFixture {
    pub(crate) foundation: scoop_hir::ImportedHirFoundation,
    pub(crate) source_foundation: scoop_hir::OdrFreeHirFoundation,
    general_interface: scoop_hir::CrossConeHirInterfaceSectionV1,
    aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    pub(crate) interface: scoop_hir::CompilerProtocolDefinitionsV1,

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
        scoop_mir::SelectedDependencyMirCallableV1::try_new(
            self.foundation.origin(),
            scoop_identity::DependencyCallableDeclarationId::Function(definition),
            scoop_identity::StrongCallableDefinitionOwner::Function(definition),
            signature,
        )
        .unwrap()
    }

    pub(crate) fn import_dependency_foundation(
        &mut self,
        coordinate: &scoop_identity::ConeCoordinate,
        foundation: &scoop_hir::CanonicalHirFoundation,
        fingerprint: u8,
    ) -> scoop_hir::ImportedHirFoundation {
        self.import_dependency_foundation_with_functions(coordinate, foundation, fingerprint, &[])
    }

    pub(crate) fn import_dependency_foundation_with_functions(
        &mut self,
        coordinate: &scoop_identity::ConeCoordinate,
        foundation: &scoop_hir::CanonicalHirFoundation,
        fingerprint: u8,
        external: &[scoop_identity::CborIdentityRecord<
            scoop_identity::PersistentFunctionId,
            scoop_identity::SourceDeclarationKey,
        >],
    ) -> scoop_hir::ImportedHirFoundation {
        let decoded: scoop_hir::DecodedHirFoundation =
            decode_canonical(&encode(foundation).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(coordinate.identity().unwrap())
            .unwrap();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        for record in external {
            pending
                .register_external_canonical_authority(record.clone())
                .unwrap();
        }
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
    trusted_core_from_source(complete_core_file(), "")
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
    trusted_core_from_source(source, "")
}

pub(crate) fn trusted_core_from_source(
    source: scoop_ast::SourceFile,
    source_text: &str,
) -> TrustedCoreFixture {
    let parsed = parsed_sources(
        super::super::core_source_identity("src/core.scoop"),
        source,
        "<core>",
        source_text,
    );
    let output = lower_core_bootstrap(&parsed).unwrap();
    let interface = scoop_hir::CompilerProtocolDefinitionsV1::from_export(&output.export).unwrap();
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
        decode_canonical(&encode(&canonical).unwrap()).unwrap();
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
    let (hir, _, _) = imported.into_parts();
    let source_foundation = scoop_hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
    let foundation =
        scoop_hir::ImportedHirFoundation::from_odr_free(source_foundation.clone(), hir);
    TrustedCoreFixture {
        general_interface,
        aliases,
        foundation,
        source_foundation,
        interface,

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

pub(crate) fn parsed_ordinary_text(source: &str) -> CurrentConeParsedSources {
    parsed_sources(
        test_source_identity("src/main.scoop"),
        scoop_parser::parse(source).unwrap(),
        "<main>",
        source,
    )
}

pub(crate) fn parsed_ordinary_text_at(
    coordinate: &ConeCoordinate,
    source: &str,
) -> CurrentConeParsedSources {
    parsed_sources(
        SourceIdentity::new(
            coordinate.identity().unwrap(),
            NormalizedSourcePath::new("src/main.scoop").unwrap(),
        )
        .unwrap(),
        scoop_parser::parse(source).unwrap(),
        "<dependency-main>",
        source,
    )
}
