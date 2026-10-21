use super::*;
use hir::SemanticTypeStructure as Shape;

#[test]
fn dependency_type_description_does_not_request_companion_initialization() {
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
            let context = hir::SemanticContext::Function(function);
            let hir::ExportParameterCalling::Required { value_type } =
                queries.signature(function).parameters[0].calling
            else {
                panic!("Box parameter")
            };
            let Shape::Nominal(nominal) = queries
                .describe_type(context.into(), value_type)
                .unwrap()
                .structure
            else {
                panic!("dependency Box")
            };
            assert_eq!(nominal.kind, hir::StaticNominalKind::Class);
            let companion = nominal.companion.unwrap();
            let resolved = queries
                .resolve_type(
                    context,
                    &scoop_parser::parse_type("Box<Int>.Companion").unwrap(),
                )
                .unwrap();
            assert_eq!(resolved, companion);
            let Shape::Nominal(companion) = queries
                .describe_type(context.into(), companion)
                .unwrap()
                .structure
            else {
                panic!("dependency companion")
            };
            assert_eq!(companion.kind, hir::StaticNominalKind::Object);
            assert_eq!(companion.arguments, nominal.arguments);
            assert_ne!(companion.declaration, nominal.declaration);
            let integer = nominal.arguments[0];
            let Shape::Nominal(integer) = queries
                .describe_type(context.into(), integer)
                .unwrap()
                .structure
            else {
                panic!("the primitive retains its actual core identity")
            };
            assert_eq!(
                integer.kind,
                hir::StaticNominalKind::Intrinsic(hir::IntrinsicTypeKind::Integer(
                    hir::IntegerKind::SIGNED_32
                ))
            );
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspector = Inspect(false);
    let output = support::with_provider(
        "public annotation class Mark\npublic class Box<T> public constructor(public val value: T) { public companion object Factory {} }",
        "@Mark fun inspect(value: Box<Int>) {}",
        &mut inspector,
    );
    assert!(inspector.0);
    assert!(
        output
            .output()
            .export
            .module()
            .imported_companion_templates
            .is_empty()
    );
}
