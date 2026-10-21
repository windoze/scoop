use super::*;

#[test]
fn documentation_products_preserve_warning_categories_and_optional_artifact() {
    let path = HostPathCarrier::from_path(std::path::Path::new("library.json")).unwrap();
    let doc_warning = diagnostic(DiagnosticSeverityV1::Warning, "unresolved link".to_owned());
    let compiler_warning = diagnostic(DiagnosticSeverityV1::Warning, "unused value".to_owned());
    let documentation =
        ScoopcDocumentationOutputV1::new(path.clone(), vec![doc_warning.clone()]).unwrap();
    for product in [
        ScoopcProductV1::Documentation(documentation.clone()),
        ScoopcProductV1::ArtifactWithDocumentation {
            artifact: Box::new(crate::test_summary()),
            documentation: documentation.clone(),
        },
    ] {
        let has_artifact = product.summary().is_some();
        let success =
            ScoopcSuccessV1::new(product, vec![compiler_warning.clone()], Vec::new()).unwrap();
        let response =
            ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([19; 16]), success);
        let decoded =
            crate::decode_response_frame(&crate::encode_response_frame(&response).unwrap())
                .unwrap();
        assert_eq!(decoded, response);
        let ScoopcResponseEnvelopeV1::Success { result, .. } = decoded else {
            panic!("documentation must return a production success");
        };
        assert_eq!(result.summary().is_some(), has_artifact);
        assert_eq!(result.warnings(), std::slice::from_ref(&compiler_warning));
        assert_eq!(result.documentation().unwrap().input(), &path);
        assert_eq!(
            result.documentation().unwrap().warnings(),
            std::slice::from_ref(&doc_warning)
        );
    }
    assert_eq!(
        ScoopcDocumentationOutputV1::new(
            path,
            vec![diagnostic(DiagnosticSeverityV1::Error, "error".to_owned())],
        )
        .unwrap_err(),
        ProtocolValidationError::SuccessContainsErrorDiagnostic
    );
}
