use super::*;

#[test]
fn pending_dependency_method_keeps_inherited_defaults_and_applied_owner_binders() {
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let pending = queries.pending_methods();
            assert_eq!(pending.len(), 1);
            let pending = &pending[0];
            for requirement in &pending.requirements {
                let hir::InterfaceMethodReference::Imported { slot, .. } = requirement.declaration
                else {
                    panic!("the requirement comes from the dependency interface")
                };
                assert_eq!(
                    queries.requirement_slot(requirement.declaration).unwrap(),
                    slot
                );
            }
            let signature =
                scoop_generator_protocol::pending_method_signature(queries, pending).unwrap();
            assert_eq!(signature["parameters"][1]["calling"]["kind"], "default");
            let parameters = queries.function_parameters(pending.function).unwrap();
            let hir::SemanticParameterCalling::Default(default) = &parameters[1].calling else {
                panic!("the implementation inherits its dependency parameter default")
            };
            let projected = queries
                .projected_default(default.definition)
                .unwrap()
                .unwrap();
            assert_ne!(
                projected.definition_origin().origin().source().cone(),
                queries.current_cone()
            );
            assert_ne!(
                projected.key().owner(),
                projected.definition_root().declaration()
            );
            assert_eq!(
                signature["parameters"][1]["calling"]["default"]["arguments"][0],
                signature["result"]
            );
            let origin = queries.signature(pending.function).origin;
            let text = "{ return second }";
            self.0 = true;
            Ok(vec![hir::GeneratedMethodBody {
                function: pending.function,
                body: scoop_parser::parse_function_body(text).unwrap(),
                source: hir::GeneratedMethodSource {
                    identity: scoop_identity::SourceIdentity::new(
                        queries.current_cone(),
                        scoop_identity::NormalizedSourcePath::new("generated/choose.scoop")
                            .unwrap(),
                    ),
                    name: "generated/choose.scoop".into(),
                    text: text.to_owned().into(),
                },
                origin,
            }])
        }
    }
    let mut inspection = Inspect(false);
    support::with_provider(
        "public interface Picker<T> { public fun choose(first: T, second: T = first): T }",
        "class Choice<T, U> : Picker<U>\n\
         fun <T, U> call(value: Choice<T, U>, item: U): U = value.choose(item)",
        &mut inspection,
    );
    assert!(inspection.0);
}
