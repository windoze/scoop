use super::types::resolve;
use super::*;

#[test]
fn constructor_queries_preserve_primary_targets_default_providers_and_source_identities() {
    let source = r#"
        annotation class Mark
        annotation class Label(val text: String)
        struct Pair<T>(@param:Label("first") val first: T, val second: T = first) {
            constructor(seed: T, unused: Int) : this(seed, seed) {}
        }
        class Row<T>(seed: T, val value: T = seed)
        class Defaulted(val value: Int = 1)
        @Mark fun inspect() { val value = Defaulted() }
    "#;
    let mut identities = Vec::new();
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let string = resolve(queries, context, "String");
        let pair = resolve(queries, context, "Pair<String>");
        let constructors = queries
            .nominal_constructors(context, pair)
            .unwrap()
            .unwrap();
        assert_eq!(constructors.len(), 2);
        assert_eq!(constructors[0].kind, hir::SemanticConstructorKind::Primary);
        assert_eq!(
            constructors[1].kind,
            hir::SemanticConstructorKind::Secondary
        );
        assert_eq!(constructors[0].result.applied, pair);
        assert_ne!(constructors[0].result.declaration, pair);
        assert_ne!(constructors[0].identity, constructors[1].identity);
        let fields = queries
            .nominal_fields(context, pair)
            .unwrap()
            .unwrap()
            .fields;
        for (index, parameter) in constructors[0].parameters.iter().enumerate() {
            assert_eq!(parameter.identity.owner, constructors[0].identity);
            assert_eq!(parameter.identity.index, index as u32);
            assert_eq!(
                parameter.target,
                hir::StaticParameterTarget::Field(fields[index].identity)
            );
            assert_eq!(parameter.value_type.applied, string);
        }
        assert!(
            constructors[1]
                .parameters
                .iter()
                .all(|parameter| parameter.target == hir::StaticParameterTarget::Plain)
        );
        assert!(
            matches!(queries.describe_annotations(constructors[0].parameters[0].annotations), hir::SemanticAnnotations::Current(annotations) if annotations.len() == 1)
        );
        let hir::SemanticParameterCalling::Default(default) =
            &constructors[0].parameters[1].calling
        else {
            panic!("declared source default")
        };
        assert_eq!(default.arguments, [string]);
        let Some(hir::SemanticDefaultDefinition::Current { template, .. }) =
            queries.default_definition(default.definition)
        else {
            panic!("original default definition")
        };
        assert_eq!(template.value_parameters[0].position, 0);
        assert!(
            matches!(template.value.kind, hir::ExprKind::Local(local) if local == template.value_parameters[0].local)
        );
        let row = resolve(queries, context, "Row<String>");
        let constructors = queries.nominal_constructors(context, row).unwrap().unwrap();
        assert_eq!(constructors.len(), 1);
        let property = &queries.nominal_properties(context, row).unwrap().unwrap()[0];
        assert_eq!(
            constructors[0].parameters[0].target,
            hir::StaticParameterTarget::Plain
        );
        assert_eq!(
            constructors[0].parameters[1].target,
            hir::StaticParameterTarget::Property(property.identity)
        );
        identities.push(constructors[0].identity);
        let defaulted = resolve(queries, context, "Defaulted");
        let constructors = queries
            .nominal_constructors(context, defaulted)
            .unwrap()
            .unwrap();
        assert_eq!(
            constructors.len(),
            1,
            "generated zero-argument adapters are not source constructors"
        );
        identities.push(constructors[0].identity);
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    for (name, identity) in ["Row", "Defaulted"].into_iter().zip(identities) {
        let (_, class) = module
            .classes
            .iter()
            .find(|(_, class)| class.name == name)
            .unwrap();
        let constructor = class
            .constructors
            .iter()
            .find(|id| {
                module.class_constructors[**id].identity_kind
                    == hir::ClassConstructorIdentityKind::Source
            })
            .unwrap();
        assert_eq!(
            identity,
            scoop_identity::CallableTemplateOrigin::Constructor(
                module.constructor_identities[*constructor]
                    .source_record()
                    .unwrap()
                    .id()
            )
        );
    }
}

#[test]
fn constructor_queries_preserve_vararg_omission_and_variant_parameter_relations() {
    let source = r#"
        annotation class Mark
        class Many<T>(vararg values: T)
        class Fallback(vararg values: Int = [1, 2])
        enum Choice<T> { Empty, Pair(T, Int), Named(val first: T, val second: T = first) }
        @Mark fun inspect() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let string = resolve(queries, context, "String");
        let many = resolve(queries, context, "Many<String>");
        let constructors = queries
            .nominal_constructors(context, many)
            .unwrap()
            .unwrap();
        let parameter = &constructors[0].parameters[0];
        let hir::SemanticParameterCalling::Vararg {
            element_type,
            omission,
        } = &parameter.calling
        else {
            panic!("vararg")
        };
        assert_eq!(element_type.applied, string);
        assert_ne!(element_type.declaration, string);
        assert!(matches!(omission, hir::SemanticVarargOmission::EmptyArray));
        let array = resolve(queries, context, "Array<String>");
        assert_eq!(parameter.value_type.applied, array);
        let fallback = resolve(queries, context, "Fallback");
        let constructors = queries
            .nominal_constructors(context, fallback)
            .unwrap()
            .unwrap();
        let hir::SemanticParameterCalling::Vararg {
            omission: hir::SemanticVarargOmission::Default(default),
            ..
        } = &constructors[0].parameters[0].calling
        else {
            panic!("vararg default")
        };
        assert!(queries.default_definition(default.definition).is_some());
        let choice = resolve(queries, context, "Choice<String>");
        let constructors = queries
            .nominal_constructors(context, choice)
            .unwrap()
            .unwrap();
        let variants = queries
            .nominal_fields(context, choice)
            .unwrap()
            .unwrap()
            .variants;
        assert_eq!(constructors.len(), variants.len());
        for (constructor, variant) in constructors.iter().zip(&variants) {
            assert_eq!(
                constructor.kind,
                hir::SemanticConstructorKind::Variant(variant.identity)
            );
            for (parameter, field) in constructor.parameters.iter().zip(&variant.fields) {
                assert_eq!(
                    parameter.target,
                    hir::StaticParameterTarget::Field(field.identity)
                );
                assert_eq!(parameter.value_type, field.value_type);
            }
        }
        let hir::SemanticParameterCalling::Default(default) =
            &constructors[2].parameters[1].calling
        else {
            panic!("variant default")
        };
        assert_eq!(default.arguments, [string]);
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
