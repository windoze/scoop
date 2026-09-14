use std::path::{Path, PathBuf};

struct TemporarySource(PathBuf);

impl TemporarySource {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "scoop-source-integer-codegen-{}.scoop",
            std::process::id()
        ));
        std::fs::write(&path, source).expect("write temporary Scoop source");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporarySource {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

pub(super) fn lower_program(
    source: &str,
) -> (
    scoop_mir::SingleConeStrongMirInput,
    scoop_lir::Module,
    String,
) {
    let temporary = TemporarySource::new(source);
    let inputs = scoopc::load_inputs(temporary.path()).expect("load compiler inputs");
    let user_index = inputs.len() - 1;
    let mut files = Vec::with_capacity(inputs.len());
    let mut diagnostics = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        match scoop_parser::parse(&input.source) {
            Ok(file) => files.push(file),
            Err(mut errors) => {
                for error in &mut errors {
                    error.reattribute_single_source(index);
                }
                diagnostics.extend(errors);
            }
        }
    }
    assert!(
        diagnostics.is_empty(),
        "source must parse:\n{}",
        scoopc::render_diagnostics(&diagnostics, &inputs, "<integer-test>", source)
    );

    let core_provider = scoop_hir::IntrinsicProviderId::from_raw(0);
    let user_provider = scoop_hir::IntrinsicProviderId::from_raw(1);
    let user_sources = scoop_ast::AllParsedSources::try_new(scoop_ast::NonEmptyVec::new(
        scoop_ast::IdentifiedParsedSource::new(
            inputs[user_index].identity.clone(),
            files[user_index].clone(),
        ),
        Vec::new(),
    ))
    .expect("the integer test has one identified user source");
    let input = scoop_hir_lower::LegacyCombinedSources::try_new(
        files[..user_index]
            .iter()
            .zip(&inputs[..user_index])
            .map(|(source, input)| scoop_hir_lower::ProviderSource {
                source,
                identity: input.identity.clone(),
                provider: core_provider,
                name: &input.name,
                source_text: &input.source,
            })
            .collect(),
        user_provider,
        user_sources,
        |identity| {
            assert_eq!(identity, &inputs[user_index].identity);
            scoop_hir_lower::CurrentSourceDetails {
                display_locator: &inputs[user_index].name,
                source_text: &inputs[user_index].source,
            }
        },
    )
    .expect("the integer test has unique source identities");
    let hir = scoop_hir_lower::lower_combined_sources(
        scoop_identity::RequestedConeKind::Executable,
        &input,
        scoop_hir_lower::IntrinsicDeclarationPolicy::default(),
    )
    .unwrap_or_else(|errors| {
        panic!(
            "source must lower to HIR:\n{}",
            scoopc::render_diagnostics(&errors, &inputs, "<integer-test>", source)
        )
    });
    let hir_production = scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.export)
        .expect("integer test HIR has a complete production section");
    let mir = scoop_mir_lower::lower(&hir.local);
    let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&mir)
        .expect("integer test MIR satisfies the strong profile");
    let production =
        scoop_mir_lower::lower_production_section(mir.cone, &hir_production, &foundation)
            .expect("integer test MIR has a complete production section");
    let mir = scoop_mir::SingleConeStrongMirInput::try_new(
        mir,
        foundation,
        production,
        scoop_mir::CoreShapeSupportSourceInput::NotCore,
    )
    .expect("integer test MIR seals as one strong input");
    let profile =
        scoop_codegen::ResolvedTargetProfile::resolve_host().expect("supported host profile");
    let lir = scoop_lir_lower::lower(&mir, profile.lir_target())
        .expect("integer test lowers to ODR-free LIR")
        .into_module();
    let llvm =
        scoop_codegen::render_llvm_ir(&lir, profile.backend()).expect("render verified LLVM IR");
    (mir, lir, llvm)
}
