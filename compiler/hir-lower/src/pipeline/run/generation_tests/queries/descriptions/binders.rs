use super::*;

#[test]
fn applied_method_json_does_not_capture_lexical_or_same_declaration_binders() {
    let source = r#"
        annotation class Mark
        interface View<T> {}
        class Box<T>(val value: T) {
            fun <U: View<T>> choose(first: T, other: U): U = other
            fun <U> copy(first: T, other: U, second: T = first, again: U = other): U = again
            @Mark fun <U> inspect(other: U): U = other
        }
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        for (text, applied_depth) in [("Box<U>", 1), ("Box<T>", 2)] {
            let ty = resolve(queries, context, text);
            let description = member_description(queries, function, ty).unwrap();
            let choose = named(&description["methods"], "choose");
            let outer = json!({"kind": "binder", "depth": applied_depth, "index": 0});
            let own = json!({"kind": "binder", "depth": 0, "index": 0});
            assert_eq!(choose["parameters"][0]["valueType"]["appliedType"], outer);
            assert_eq!(choose["parameters"][1]["valueType"]["appliedType"], own);
            assert_eq!(choose["result"]["appliedType"], own);
            assert_eq!(
                choose["typeParameters"][0]["applied"]["bounds"]["bounds"][0]["valueType"]["arguments"]
                    [0],
                outer
            );
            let copy = named(&description["methods"], "copy");
            for index in [2, 3] {
                assert_eq!(
                    copy["parameters"][index]["calling"]["default"]["arguments"],
                    json!([outer, own])
                );
            }
            let same = named(&description["methods"], "inspect");
            assert_eq!(same["receiver"]["appliedType"]["arguments"][0], outer);
            assert_eq!(same["parameters"][0]["valueType"]["appliedType"], own);
            assert_eq!(same["result"]["appliedType"], own);
            assert_eq!(
                member_description(queries, function, ty).unwrap(),
                description
            );
        }
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
