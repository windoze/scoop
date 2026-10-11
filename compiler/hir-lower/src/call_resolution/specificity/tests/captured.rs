use super::*;

#[test]
fn forwarding_keeps_captured_alias_owners_fixed() {
    let mut state = Lowerer::new();
    let outer = parameter(10, 0);
    let outer_ty = state.intern_type(Type::Param(outer.id));
    state.type_params_in_scope.push(outer);
    let integer = state.integer_type(hir::IntegerKind::SIGNED_32);
    let any = state.intern_type(Type::Any);
    let specific =
        DeclarationForwardingView::parameter_groups(&[], &[], &[outer_ty, integer]).to_owned();
    let general =
        DeclarationForwardingView::parameter_groups(&[], &[], &[outer_ty, any]).to_owned();
    assert!(state.declaration_forwards(specific.as_view(), general.as_view()));
    assert!(!state.declaration_forwards(general.as_view(), specific.as_view()));
    let different_owner =
        DeclarationForwardingView::parameter_groups(&[], &[], &[state.string, any]).to_owned();
    assert!(!state.declaration_forwards(specific.as_view(), different_owner.as_view()));

    let first = parameter(11, 0);
    let second = parameter(12, 0);
    let first_ty = state.intern_type(Type::Param(first.id));
    let second_ty = state.intern_type(Type::Param(second.id));
    let left = state.intern_type(Type::Tuple(vec![outer_ty, first_ty]));
    let right = state.intern_type(Type::Tuple(vec![outer_ty, second_ty]));
    let specific =
        DeclarationForwardingView::parameter_groups(&[], &[first], &[left, integer]).to_owned();
    let general =
        DeclarationForwardingView::parameter_groups(&[], &[second], &[right, any]).to_owned();
    assert!(state.declaration_forwards(specific.as_view(), general.as_view()));
    assert!(!state.declaration_forwards(general.as_view(), specific.as_view()));
}
