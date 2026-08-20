use crate::tests::{ok, only_function};
use scoop_ast::{TypeRef, TypeRefKind};

fn annotation(ty: &TypeRef, name: &str) {
    assert_eq!(ty.annotations.len(), 1);
    assert_eq!(ty.annotations[0].name.to_string(), name);
}

#[test]
fn type_annotations_distinguish_nullable_layers() {
    let file = ok("fun f(a: @Outer Int?, b: (@Inner Int)?, c: @Top() (@Mid Int?)?) {}");
    let parameters = &only_function(&file).params;
    annotation(&parameters[0].ty, "Outer");
    let TypeRefKind::Nullable(inner) = &parameters[0].ty.kind else {
        panic!("nullable type");
    };
    assert!(inner.annotations.is_empty());
    assert!(parameters[1].ty.annotations.is_empty());
    let TypeRefKind::Nullable(inner) = &parameters[1].ty.kind else {
        panic!("nullable type");
    };
    annotation(inner, "Inner");
    annotation(&parameters[2].ty, "Top");
    let TypeRefKind::Nullable(inner) = &parameters[2].ty.kind else {
        panic!("nullable type");
    };
    annotation(inner, "Mid");
}

#[test]
fn type_annotations_distinguish_nullable_receiver_layers() {
    let file = ok("fun @Outer String?.outer() {}");
    let receiver = only_function(&file).receiver_ty.as_ref().unwrap();
    annotation(receiver, "Outer");
    let TypeRefKind::Nullable(inner) = &receiver.kind else {
        panic!("nullable receiver");
    };
    assert!(inner.annotations.is_empty());
    let file = ok("fun (@Inner String)?.inner() {}");
    let receiver = only_function(&file).receiver_ty.as_ref().unwrap();
    assert!(receiver.annotations.is_empty());
    let TypeRefKind::Nullable(inner) = &receiver.kind else {
        panic!("nullable receiver");
    };
    annotation(inner, "Inner");
    ok("val @Outer String?.outer: Int get() = 42");
    ok("val (@Inner String)?.inner: Int get() = 42");
}

#[test]
fn type_annotations_keep_function_and_receiver_positions() {
    let file =
        ok("fun f(value: @Whole() suspend (@Receiver Int).(@Parameter String) -> @Result Int) {}");
    let ty = &only_function(&file).params[0].ty;
    annotation(ty, "Whole");
    let TypeRefKind::Function(signature) = &ty.kind else {
        panic!("function type");
    };
    assert!(signature.is_suspend);
    annotation(signature.receiver.as_ref().unwrap(), "Receiver");
    annotation(&signature.parameters[0], "Parameter");
    annotation(&signature.return_type, "Result");
}

#[test]
fn type_annotations_cover_arguments_tuples_aliases_and_bounds() {
    let file = ok(
        "fun <T: @Bound Any> f(value: @Root Array<@Tuple() (@Left T, @Right Int)>): @Result T {} ",
    );
    let function = only_function(&file);
    annotation(&function.params[0].ty, "Root");
    let TypeRefKind::Generic(_, arguments) = &function.params[0].ty.kind else {
        panic!("generic type");
    };
    annotation(&arguments[0], "Tuple");
    let TypeRefKind::Tuple(elements) = &arguments[0].kind else {
        panic!("tuple type");
    };
    annotation(&elements[0], "Left");
    annotation(&elements[1], "Right");
    ok("typealias A<T> = @Alias Array<@Argument T>");
    ok("interface I {} class C: @Parent I {}");
    ok("fun (@Receiver Int).extension(): @Result Int = this");
    ok("fun f(value: @field:Wrong Int) {}");
}
