use super::*;
use std::cell::Cell;

fn functions(count: usize) -> String {
    (0..count)
        .map(|index| format!("fun item{index}(value: Boolean): Boolean = value\n"))
        .collect()
}

#[test]
fn declaration_collection_stops_between_actual_source_declarations() {
    let source = functions(128);
    let (mut lowerer, files) = input(&source);
    let checks = Cell::new(0);
    let limit = files[0].declarations.len() + 6;
    let result = lowerer.collect_declaration_inputs_with_cancellation(
        files.iter().map(ast::SourceFile::header).collect(),
        files.iter().enumerate().flat_map(|(file, source)| {
            source
                .declarations
                .iter()
                .map(move |declaration| (file, ast::DeclarationInput::Complete(declaration)))
        }),
        None,
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == limit {
                Err(())
            } else {
                Ok(())
            }
        },
    );
    assert!(matches!(result, Err(PhaseFailure::Cancelled(()))));
    assert_eq!(checks.get(), limit);
    let declared = lowerer
        .functions
        .iter()
        .filter(|(_, function)| function.name.starts_with("item"))
        .count();
    assert!(declared > 0 && declared < 128, "declared {declared}");
    assert!(
        lowerer.diagnostics.is_empty(),
        "cancellation is not a diagnostic"
    );
}

#[test]
fn signature_resolution_stops_before_resolving_later_function_headers() {
    let source = functions(512);
    let (mut lowerer, files) = input(&source);
    let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
    let checks = Cell::new(0);
    let limit = declarations.function_headers.len() / 2;
    let result = lowerer.resolve_source_signatures_with_cancellation(&declarations, &|| {
        checks.set(checks.get() + 1);
        if checks.get() == limit {
            Err(())
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(PhaseFailure::Cancelled(()))));
    assert_eq!(checks.get(), limit);
    let resolved = declarations
        .function_headers
        .iter()
        .filter(|(id, header, _)| {
            header.name.text.starts_with("item") && lowerer.signatures.contains_key(id)
        })
        .count();
    assert!(resolved > 0 && resolved < 512, "resolved {resolved}");
    assert!(!lowerer.declaration_surface.is_frozen());
    assert!(
        lowerer.diagnostics.is_empty(),
        "cancellation is not a diagnostic"
    );
}

#[test]
fn duplicate_signature_cancellation_does_not_publish_a_partial_rejection_set() {
    let source = functions(16);
    let (mut lowerer, files) = input(&source);
    let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
    lowerer.resolve_source_signatures(&declarations).unwrap();
    lowerer.declaration_surface = Default::default();
    let checks = Cell::new(0);
    let result = lowerer.validate_and_freeze_duplicate_signatures_with_cancellation(
        &declarations.function_headers,
        &declarations.method_headers,
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 4 { Err(()) } else { Ok(()) }
        },
    );
    assert_eq!(result, Err(()));
    assert_eq!(checks.get(), 4);
    assert!(!lowerer.declaration_surface.is_frozen());
}
