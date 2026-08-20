use super::*;

#[test]
fn static_shapes_substitute_nested_types_without_reapplying_open_arguments() {
    with_shapes(
        r#"
            package shapes
            public annotation class Label(val text: String)
            @Label("box")
            public struct Box<T>(@Label("item") val value: T, val pair: (T, Int), val apply: (T) -> T)
        "#,
        r#"
            import shapes.Box
            public fun stringBox(value: Box<String>): Box<String> = value
        "#,
        |module, world, imported| {
            let shape = nominal(module, "stringBox");
            assert_eq!(shape.kind(), hir::StaticNominalKind::Struct);
            assert_eq!(
                annotation_text(shape.annotations(world), module, world),
                "box"
            );
            let fields = shape.fields().collect::<Vec<_>>();
            assert_eq!(
                fields.iter().map(|f| f.name()).collect::<Vec<_>>(),
                ["value", "pair", "apply"]
            );
            let string = signature(module, shape.arguments()[0]);
            assert_eq!(
                fields[0].value_type().signature(module, &[]).unwrap(),
                string
            );
            assert_eq!(
                annotation_text(fields[0].annotations(world), module, world),
                "item"
            );
            let SignatureTypeKey::Tuple(elements) =
                fields[1].value_type().signature(module, &[]).unwrap()
            else {
                panic!("tuple field")
            };
            assert_eq!(elements.as_slice()[0], string);
            let SignatureTypeKey::Function {
                parameters, result, ..
            } = fields[2].value_type().signature(module, &[]).unwrap()
            else {
                panic!("function field")
            };
            assert_eq!(parameters, vec![string.clone()]);
            assert_eq!(*result, string);
            let primary = shape.primary_constructor(world).unwrap();
            for (parameter, field) in primary.parameters().zip(&fields) {
                assert_eq!(
                    parameter.target(),
                    hir::StaticParameterTarget::Field(field.identity())
                );
                assert_eq!(
                    parameter.value_type(&[]).unwrap(),
                    field.value_type().signature(module, &[]).unwrap()
                );
            }
            let definition = if imported {
                &module.loaded_struct_definitions[&shape.declaration()].definition
            } else {
                let hir::StaticNominalOrigin::Current(hir::NominalOwner::Struct(id)) =
                    shape.origin()
                else {
                    panic!("current struct")
                };
                &module.structs[id].definition
            };
            let ty = module.struct_applications[definition.self_application].canonical_type;
            let hir::StaticTypeShape::Nominal(open) = module.static_type_shape(ty, &[]).unwrap()
            else {
                panic!("open struct")
            };
            let binder = hir::HirSignatureBinder {
                parameter: open.parameters()[0].id,
                depth: 0,
                index: 0,
            };
            assert_eq!(
                open.fields()
                    .next()
                    .unwrap()
                    .value_type()
                    .signature(module, &[binder])
                    .unwrap(),
                SignatureTypeKey::Binder { depth: 0, index: 0 }
            );
            let parameter_ty = open.arguments()[0];
            assert!(module.static_type_shape(parameter_ty, &[]).is_err());
            let hir::StaticTypeShape::Parameter(parameter) = module
                .static_type_shape(parameter_ty, open.parameters())
                .unwrap()
            else {
                panic!("parameter shape")
            };
            assert_eq!(parameter.id, binder.parameter);
        },
    );
}

#[test]
fn static_enum_shapes_keep_variant_layers_and_constructor_defaults() {
    with_shapes(
        r#"
            package shapes
            public annotation class Label(val text: String)
            public enum Choice<T> {
                @Label("empty") Empty,
                Named(@Label("named") val value: T, val count: Int = 4),
                Other(val value: T),
                Pair(T, Int)
            }
        "#,
        r#"
            import shapes.Choice
            public fun choice(value: Choice<String>): Choice<String> = value
        "#,
        |module, world, imported| {
            let shape = nominal(module, "choice");
            assert_eq!(shape.kind(), hir::StaticNominalKind::Enum);
            assert_eq!(shape.fields().len(), 0);
            let variants = shape.variants().collect::<Vec<_>>();
            assert_eq!(
                variants.iter().map(|v| v.name()).collect::<Vec<_>>(),
                ["Empty", "Named", "Other", "Pair"]
            );
            assert_eq!(
                annotation_text(variants[0].annotations(world), module, world),
                "empty"
            );
            assert_eq!(variants[0].constructor(world).parameters().len(), 0);
            let field = variants[1].fields().next().unwrap();
            assert_ne!(
                field.identity(),
                variants[2].fields().next().unwrap().identity()
            );
            assert_eq!(
                annotation_text(field.annotations(world), module, world),
                "named"
            );
            assert_eq!(
                field.value_type().signature(module, &[]).unwrap(),
                signature(module, shape.arguments()[0])
            );
            let parameters = variants[1]
                .constructor(world)
                .parameters()
                .collect::<Vec<_>>();
            assert_eq!(
                parameters[0].target(),
                hir::StaticParameterTarget::Field(field.identity())
            );
            assert!(parameters[0].default().is_none());
            assert_eq!(
                matches!(
                    parameters[1].default().unwrap(),
                    hir::StaticDefaultReference::Dependency { .. }
                ),
                imported
            );
            assert_eq!(variants[3].style(), hir::VariantStyle::Positional);
        },
    );
}
