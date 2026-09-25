use super::*;

mod binding;

pub(super) struct FlowAuthority<'a> {
    observation: Observation<'a>,
    field: PersistentFieldId,
}
impl ProtectedDefaultLocalDataFlowSemanticAuthority<&'static str> for FlowAuthority<'_> {
    fn default_binding_struct_field_index(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        declaration: PersistentFieldId,
        owner: &SignatureTypeKey,
    ) -> Result<u32, &'static str> {
        assert!(std::ptr::eq(self.observation.template, template));
        self.observation.calls += 1;
        if declaration == self.field && matches!(owner, SignatureTypeKey::Nominal(_)) {
            Ok(7)
        } else {
            Err("unknown binding field")
        }
    }
}
#[test]
fn protected_flow_preserves_definition_order_and_constructor_receiver_unavailability() {
    let f = Fixture::new();
    let declared = LocalValueSelector::LocalDeclaration {
        path: path(StructuralDefinitionSiteRole::LocalDeclaration, 1),
    };
    let read = || {
        statement(
            &f,
            DefaultStatementKindV1::Expr(Box::new(expression(
                &f,
                DefaultExpressionKindV1::Local(declared.clone()),
                value(&f),
            ))),
        )
    };
    let declare = || {
        statement(
            &f,
            DefaultStatementKindV1::ValDecl {
                pattern: DefaultPatternV1::binding(declared.clone()),
                init: Box::new(unit(&f)),
            },
        )
    };
    for valid in [true, false] {
        let statements = if valid {
            vec![declare(), read()]
        } else {
            vec![read(), declare()]
        };
        let template = template(
            &f,
            vec![local(&f, declared.clone())],
            vec![],
            statements,
            unit(&f),
        );

        let path = WirePath::root();
        let mut authority = FlowAuthority {
            observation: Observation::new(&template, &path),
            field: f.field,
        };
        let result = template.validate_local_data_flow_semantics(&mut authority, &path);
        if valid {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ExportDefaultLocalDataFlowValidationError::Local {
                    error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
                    ..
                })
            ));
        }
    }
    let mut constructor = template(
        &f,
        vec![local(&f, LocalValueSelector::This)],
        vec![],
        vec![],
        expression(
            &f,
            DefaultExpressionKindV1::Local(LocalValueSelector::This),
            value(&f),
        ),
    );
    constructor.key = ProtectedDefaultTemplateKeyV1::try_new(
        CallableTemplateOrigin::Constructor(f.constructor),
        0,
    )
    .unwrap();
    constructor.definition_root = PersistentLexicalRootV1::Constructor(f.constructor);

    let path = WirePath::root();
    let mut authority = FlowAuthority {
        observation: Observation::new(&constructor, &path),
        field: f.field,
    };
    assert!(matches!(
        constructor.validate_local_data_flow_semantics(&mut authority, &path),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
            ..
        })
    ));
}
