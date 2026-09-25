use super::*;

#[test]
fn explicit_cast_targets_match_plain_and_optional_results() {
    let fixture = Fixture::new();
    let string = core(DefaultOperationCoreTypeV1::String);
    for optional in [false, true] {
        let result = if optional { ty(100) } else { string.clone() };
        let operand = expression(
            &fixture,
            DefaultExpressionKindV1::StringLiteral {
                value: "value".to_owned(),
                owner: DefaultStringOwnerV1::CurrentInstantiation,
            },
            string.clone(),
        );
        let cast = |checked_type| {
            expression(
                &fixture,
                DefaultExpressionKindV1::Cast {
                    operand: Box::new(operand.clone()),
                    checked_type,
                    optional: CanonicalBooleanV1::from(optional),
                },
                result.clone(),
            )
        };
        let mut authority = Authority::new();
        authority
            .relations
            .push(DefaultOperationTypeRelationV1::RuntimeTypeCheck);
        if optional {
            authority.applications.push((
                result.clone(),
                DefaultCoreApplicationV1::Option {
                    element: string.clone(),
                },
            ));
        }
        let valid = template(
            &fixture,
            cast(string.clone()),
            Vec::new(),
            Vec::new(),
            false,
        );
        assert_eq!(validate(&valid, &mut authority), Ok(()));
        assert!(
            authority
                .relation_queries
                .iter()
                .any(|(relation, _, target)| *relation
                    == DefaultOperationTypeRelationV1::RuntimeTypeCheck
                    && target == &string)
        );
        let invalid = template(
            &fixture,
            cast(core(DefaultOperationCoreTypeV1::Boolean)),
            Vec::new(),
            Vec::new(),
            false,
        );
        assert!(
            matches!(validate(&invalid, &mut authority), Err(ExportDefaultOperationTypingValidationError::Type { site, expected, actual }) if site.operation() == DefaultExpressionOperationV1::Cast && site.role() == DefaultOperationValueRoleV1::CheckedType && *expected == string && *actual == core(DefaultOperationCoreTypeV1::Boolean))
        );
    }
}
