use super::*;
use scoop_identity::SignatureTypeKey as Key;

#[test]
fn member_types_use_their_declaration_binders_and_restore_lexical_context() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let caller_t = resolve(queries, context, "T");
        let box_type = resolve(queries, context, "Box<Int>");
        let Shape::Nominal(nominal) = queries
            .describe_type(context.into(), box_type)
            .unwrap()
            .structure
        else {
            panic!("Box declaration")
        };
        let fields = queries.nominal_fields(context, box_type).unwrap().unwrap();
        let field = fields.fields[0].value_type;
        let nominal_binders = nominal.declaration_binders();
        assert!(
            queries
                .type_signature(context.into(), field.declaration)
                .is_err()
        );
        assert_eq!(
            queries
                .type_signature((&nominal_binders).into(), field.declaration)
                .unwrap(),
            Key::Binder { depth: 0, index: 0 }
        );
        let Shape::Parameter(parameter) = queries
            .describe_type((&nominal_binders).into(), field.declaration)
            .unwrap()
            .structure
        else {
            panic!("actual Box parameter")
        };
        assert_eq!(parameter.id, nominal.parameters[0].id);
        assert_ne!(field.declaration, caller_t);

        let methods = queries.nominal_methods(context, box_type).unwrap().unwrap();
        let method = &methods[0];
        let binders = method.declaration_binders();
        assert_eq!(
            queries
                .type_signature(
                    (&binders).into(),
                    method.parameters[0].value_type.declaration
                )
                .unwrap(),
            Key::Binder { depth: 1, index: 0 }
        );
        assert_eq!(
            queries
                .type_signature((&binders).into(), method.result.declaration)
                .unwrap(),
            Key::Binder { depth: 0, index: 0 }
        );
        let Shape::Parameter(own) = queries
            .describe_type((&binders).into(), method.result.declaration)
            .unwrap()
            .structure
        else {
            panic!("actual method parameter")
        };
        let bound = own.nominal_bounds_in_source_order()[0].ty();
        let Key::NominalApplication { arguments, .. } =
            queries.type_signature((&binders).into(), bound).unwrap()
        else {
            panic!("View of the outer Box parameter")
        };
        assert_eq!(arguments.as_slice(), [Key::Binder { depth: 1, index: 0 }]);
        assert!(
            queries
                .describe_type(context.into(), method.result.declaration)
                .is_err()
        );
        assert!(queries.type_signature((&binders).into(), caller_t).is_err());
        assert_eq!(
            queries
                .describe_type(context.into(), caller_t)
                .unwrap()
                .name,
            "T"
        );
        Ok(Vec::new())
    });
    compile(
        "annotation class Mark\ninterface View<T> {}\nstruct Box<T>(val value: T) { fun <U: View<T>> choose(first: T, second: U): U = second }\n@Mark fun <T> inspect(value: T) {}",
        &mut driver,
    ).unwrap();
}
