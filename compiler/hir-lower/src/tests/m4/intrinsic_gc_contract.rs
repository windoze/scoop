use super::*;

#[test]
fn gc_control_intrinsics_remain_required_before_metadata_production() {
    for (name, intrinsic) in [("gcCollect", "rt_gc_collect"), ("gcStats", "rt_gc_stats")] {
        let mut core = complete_core_file();
        core.declarations
            .retain(|decl| !matches!(decl, Decl::Function(function) if function.name.text == name));
        let expected_span = core.span;
        let errors = lower_core_source(core).unwrap_err();
        let message = format!("scoop.core must define exactly one `{intrinsic}` intrinsic");
        let error = errors
            .iter()
            .find(|error| error.message == message)
            .expect("missing GC control role must be diagnosed");
        assert_eq!(error.file, 0);
        assert_eq!(error.span, Some(expected_span));
    }
}

#[test]
fn gc_control_intrinsics_reject_signature_tampering_at_the_declaration() {
    for (name, intrinsic) in [("gcCollect", "rt_gc_collect"), ("gcStats", "rt_gc_stats")] {
        for invalid in [
            InvalidShape::Result,
            InvalidShape::Parameters,
            InvalidShape::Generic,
            InvalidShape::Suspend,
            InvalidShape::Receiver,
        ] {
            let mut core = complete_core_file();
            let function = core
                .declarations
                .iter_mut()
                .find_map(|decl| match decl {
                    Decl::Function(function) if function.name.text == name => Some(function),
                    _ => None,
                })
                .unwrap();
            let span = Span::new(120, 180);
            function.span = span;
            match invalid {
                InvalidShape::Result => function.return_ty = Some(ty_named("Int")),
                InvalidShape::Parameters => {
                    let Decl::Function(replacement) =
                        intrinsic_fun("unused", intrinsic, vec![("value", ty_named("Int"))], None)
                    else {
                        unreachable!()
                    };
                    function.params = replacement.params;
                }
                InvalidShape::Generic => function.type_params.push(type_param("T")),
                InvalidShape::Suspend => function.is_suspend = true,
                InvalidShape::Receiver => function.receiver_ty = Some(ty_named("Int")),
            }
            let errors = lower_core_source(core).unwrap_err();
            let message = format!("malformed core GC control intrinsic `{intrinsic}`");
            let error = errors
                .iter()
                .find(|error| error.message == message)
                .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
            assert_eq!(error.file, 0);
            assert_eq!(error.span, Some(span));
        }
    }
}

#[derive(Clone, Copy)]
enum InvalidShape {
    Result,
    Parameters,
    Generic,
    Suspend,
    Receiver,
}

fn lower_core_source(source: SourceFile) -> Result<hir::Output, Vec<Diagnostic>> {
    let identity = core_source_identity("src/gc-contract.scoop");
    let parsed = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(identity.clone(), source),
            vec![],
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), String::new()),
            vec![],
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(identity, std::path::PathBuf::from("<core>")),
            vec![],
        ),
    )
    .unwrap();
    let input = crate::CoreBootstrapSources::try_new(&parsed).unwrap();
    crate::lower_core_bootstrap(&input)
}
