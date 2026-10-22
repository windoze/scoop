use super::*;

#[test]
fn query_projection_reads_queued_dependency_signatures_without_completing_them() {
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let function = queries
                .annotations()
                .targets
                .iter()
                .find_map(|target| match target.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let Some(Availability::Available(body)) = queries
                .projected_body(hir::SemanticCallable::Current(function))
                .unwrap()
            else {
                panic!("consumer body");
            };
            assert_eq!(body.parameters().len(), 1);
            let hir::DefaultStatementKindV1::Return(value) =
                body.statements().last().unwrap().kind()
            else {
                panic!("return");
            };
            assert_eq!(value.result_type(), body.result());
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        "public annotation class Mark\npublic class Box<T> public constructor(public val value: T)\npublic fun <T> identity(value: T): T = value",
        "@Mark fun run(value: Int): Int = identity(Box<Int>(value).value)",
        &mut inspect,
    );
    assert!(inspect.0);
}
