use super::*;
use hir::{DefaultOperationEntityShapeV1 as Shape, DefaultSourceCallableOperationError as Error};
use scoop_identity::{NonEmptyVec, OptionalSignatureType, SignatureTypeKey as Type};
use scoop_wire::WirePath;
mod adapters;
mod generics;
mod rejection;
mod resources;
mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-operation-signatures/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-operation-signatures/combined.scoop"
));

#[test]
fn member_operation_signatures_preserve_receivers_accessors_effects_and_varargs() {
    with_core_source(SOURCE, |output, bound, core| {
        let int = Type::Nominal(core.integer(hir::IntegerKind::SIGNED_32).persistent());
        let unit = Type::Nominal(core.unit().persistent());
        let cell_type = template(output, "cell", 0).result().clone();
        let mut dump = Vec::new();
        for name in [
            "method",
            "inherited",
            "generic",
            "getter",
            "setter",
            "variadicCall",
            "suspended",
        ] {
            let generic = matches!(name, "generic" | "suspended");
            let template = template(
                output,
                name,
                if generic || name == "variadicCall" {
                    2
                } else {
                    1
                },
            );
            let callables = callables(&template);
            assert_eq!(
                callables.len(),
                if name == "setter" { 2 } else { 1 },
                "{name}"
            );
            for reference in callables {
                let shape = bound
                    .members()
                    .default_member_callable_shape(&reference, &mut meter(), &WirePath::root())
                    .unwrap();
                assert!(shape.captures().is_empty());
                assert_eq!(shape.receiver(), Some(&cell_type));
                let expected_effect = if name == "suspended" {
                    scoop_identity::Effect::Suspend
                } else {
                    scoop_identity::Effect::Ordinary
                };
                assert_eq!(shape.effect(), expected_effect);
                let result = if shape.result() == &unit {
                    "Unit"
                } else if generic {
                    assert_eq!(shape.result(), template.result());
                    assert!(matches!(
                        shape.result(),
                        Type::Binder { depth: 0, index: 0 }
                    ));
                    "T"
                } else {
                    assert_eq!(shape.result(), &int);
                    "Int"
                };
                let vararg;
                let parameters = match name {
                    "getter" => &[][..],
                    "setter" if result == "Int" => &[][..],
                    "variadicCall" => {
                        let [argument] = reference.type_arguments() else {
                            panic!("vararg own argument");
                        };
                        assert_eq!(argument, &Type::Binder { depth: 0, index: 0 });
                        vararg = Type::NominalApplication {
                            origin: core.array().persistent(),
                            arguments: NonEmptyVec::from_first(argument.clone(), []),
                        };
                        std::slice::from_ref(&vararg)
                    }
                    "generic" | "suspended" => std::slice::from_ref(template.result()),
                    _ => std::slice::from_ref(&int),
                };
                assert_eq!(shape.parameters(), parameters);
                dump.push(format!(
                    "{name}: {:?} receiver=Cell captures=0 parameters={} result={result}",
                    shape.effect(),
                    parameters.len()
                ));
            }
        }
        for (name, arity) in [("cell", 1), ("packet", 1), ("variant", 1), ("empty", 0)] {
            let template = template(output, name, 0);
            let reference = constructor(&template);
            let Shape::Constructor {
                owner_type,
                parameters,
            } = bound
                .default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root())
                .unwrap()
            else {
                panic!("constructor shape");
            };
            assert_eq!(&owner_type, template.result());
            assert_eq!(parameters, vec![int.clone(); arity]);
            dump.push(format!("{name}: constructor parameters={arity}"));
        }
        dump.sort();
        assert_eq!(
            dump.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-operation-signatures/standalone.snap"
            ))
        );
    });
}
