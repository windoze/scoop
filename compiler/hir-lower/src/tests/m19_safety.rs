use super::*;
use std::collections::BTreeMap;

fn lower_source(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn constructor_safety_survives_concretization_and_constructor_bodies_lower_to_mir() {
    for (source, expected) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/safety.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/safety.snap"
            )),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/generic.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/generic.snap"
            )),
        ),
    ] {
        let output = lower_source(source).unwrap();
        let mut declared = BTreeMap::new();
        let mut rows = Vec::new();
        for (id, constructor) in output.export.class_constructors.iter() {
            let name = &output.export.classes[constructor.owner].name;
            if !matches!(
                name.as_str(),
                "Base" | "Child" | "Secondary" | "Box" | "Mixed"
            ) {
                continue;
            }
            let id = output.export.constructor_identities[id]
                .source_record()
                .unwrap()
                .id();
            declared.insert(id, constructor.safety);
            rows.push(format!(
                "class {name}/{}: {:?}\n",
                constructor.parameters.len(),
                constructor.safety
            ));
        }
        for (id, constructor) in output.export.struct_constructors.iter() {
            let name = &output.export.structs[constructor.owner].name;
            if !matches!(name.as_str(), "Value" | "Pair") {
                continue;
            }
            declared.insert(
                output.export.constructor_identities[id].id(),
                constructor.safety,
            );
            rows.push(format!(
                "struct {name}/{}: {:?}\n",
                constructor.parameters.len(),
                constructor.safety
            ));
        }
        let mut materialized = 0;
        for (identity, safety) in output
            .local
            .class_constructors
            .iter()
            .map(|(_, c)| (c.materialization, c.safety))
            .chain(
                output
                    .local
                    .struct_constructors
                    .iter()
                    .map(|(_, c)| (c.materialization, c.safety)),
            )
        {
            let scoop_identity::CallableTemplateOwner::Constructor(id) = identity.template() else {
                continue;
            };
            if let Some(expected) = declared.get(&id) {
                assert_eq!(&safety, expected);
                materialized += 1;
            }
        }
        assert_eq!(materialized, declared.len());
        rows.sort();
        assert_eq!(rows.concat(), expected);
        scoop_mir_lower::lower(&output.local).unwrap();
    }
}

#[test]
fn compiler_exception_constructors_cannot_be_unsafe() {
    for name in ["Throwable", "ArithmeticException"] {
        let mut core = complete_core_file();
        let declaration = core
            .declarations
            .iter_mut()
            .find_map(|declaration| match declaration {
                ast::Decl::Class(class) if class.name.text == name => Some(class),
                _ => None,
            })
            .unwrap();
        let ast::ClassConstructorDecl::Declared(constructor) = &mut declaration.constructor else {
            panic!("explicit core constructor");
        };
        constructor.annotations.push(ast::Annotation {
            name: ident("Unsafe"),
            args: vec![],
            span: declaration.span,
        });
        let errors = lower(&[core, scoop_parser::parse("fun main() {}").unwrap()]).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message
                    == format!("compiler exception constructor `{name}` must be safe")),
            "{errors:?}"
        );
    }
}
