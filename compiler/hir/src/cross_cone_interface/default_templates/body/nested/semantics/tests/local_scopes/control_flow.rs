use super::*;
use crate::{
    DefaultCatchV1, DefaultPatternV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenGuardV1, DefaultWhenV1, OptionalDefaultWhenGuardV1,
};
use scoop_identity::LocalValueSelector;

fn regions(f: &LocalFixture, declare_in_each_region: bool) -> Vec<DefaultStatementV1> {
    let calls = || {
        let mut statements = Vec::new();
        if declare_in_each_region {
            statements.push(f.declaration());
        }
        statements.push(f.call_statement());
        statements
    };
    let loop_value = statement(
        &f.fixture,
        DefaultStatementKindV1::While {
            condition_setup: vec![f.declaration()],
            condition: Box::new(f.call()),
            body: calls(),
        },
    );
    let when_value = DefaultWhenV1::try_new(
        unit(&f.fixture),
        vec![
            DefaultWhenArmV1::try_new(
                DefaultPatternV1::wildcard(),
                OptionalDefaultWhenGuardV1::present(
                    DefaultWhenGuardV1::try_new(vec![f.declaration()], f.call()).unwrap(),
                ),
                calls(),
                origin(&f.fixture),
            )
            .unwrap(),
        ],
        DefaultWhenFallbackV1::try_else(calls()).unwrap(),
    )
    .unwrap();
    let try_value = DefaultTryV1::try_new(
        vec![f.declaration(), f.call_statement()],
        vec![
            DefaultCatchV1::try_new(
                catch_local(&f.fixture),
                binder(0),
                calls(),
                origin(&f.fixture),
            )
            .unwrap(),
        ],
        OptionalDefaultStatementListV1::try_present(calls()).unwrap(),
    )
    .unwrap();
    vec![
        loop_value,
        statement(
            &f.fixture,
            DefaultStatementKindV1::When(Box::new(when_value)),
        ),
        statement(&f.fixture, DefaultStatementKindV1::Try(Box::new(try_value))),
    ]
}

#[test]
fn evaluation_and_control_regions_do_not_merge_or_leak_local_declarations() {
    let f = LocalFixture::new();
    for region in regions(&f, true) {
        let valid = template_with_body(&f.fixture, vec![region.clone()], unit(&f.fixture));
        let mut authority = f.authority();
        validate_body(&valid, &mut authority).unwrap();
        occurrences::assert_source_index_order(&valid, &authority);
        let invalid = template_with_body(&f.fixture, vec![region], f.call());
        assert_missing(&f, &invalid);
    }
    // A condition/guard/try declaration cannot authorize a later sibling region.
    for region in regions(&f, false) {
        let invalid = template_with_body(&f.fixture, vec![region], unit(&f.fixture));
        assert_missing(&f, &invalid);
    }
}

fn assert_missing(f: &LocalFixture, template: &ExportDefaultTemplateV1) {
    assert_eq!(
        validate_body(template, &mut f.authority()).unwrap_err(),
        DefaultNestedCallableAbiValidationError::MissingLocalFunction {
            declaration: f.descriptor.declaration(),
            site: DefaultNestedCallableLocalUseV1::DirectCall,
        }
    );
}

fn catch_local(f: &Fixture) -> LocalValueSelector {
    LocalValueSelector::LocalDeclaration {
        path: child_path(
            template(f).definition_path(),
            StructuralDefinitionSiteRole::LocalDeclaration,
            42,
        ),
    }
}

fn template_with_body(
    f: &Fixture,
    statements: Vec<DefaultStatementV1>,
    value: DefaultExpressionV1,
) -> ExportDefaultTemplateV1 {
    let local = crate::TemplateLocalRecordV1::try_new(
        catch_local(f),
        binder(0),
        CanonicalBooleanV1::False,
        crate::TemplateLocalDefinitionV1::Source(origin(f)),
    )
    .unwrap();
    template_with_locals(f, vec![local], statements, value)
}
