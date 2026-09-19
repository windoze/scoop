use super::*;

struct OperationAuthority<'a> {
    observation: Observation<'a>,
    unit: SignatureTypeKey,
}
impl ProtectedDefaultOperationTypingSemanticAuthority<&'static str> for OperationAuthority<'_> {
    fn canonical_default_operation_type(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SignatureTypeKey, &'static str> {
        self.observation.check(template, meter, path)?;
        if role == DefaultOperationCoreTypeV1::Unit {
            Ok(self.unit.clone())
        } else {
            Err("unknown core role")
        }
    }
    fn classify_default_core_application(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _value: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, &'static str> {
        self.observation.check(template, meter, path)?;
        Ok(None)
    }
    fn default_operation_entity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, &'static str> {
        self.observation.check(template, meter, path)?;
        Err("unknown entity")
    }
    fn default_operation_type_relation(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, &'static str> {
        self.observation.check(template, meter, path)?;
        Ok(source == target)
    }
    fn validate_default_operation_intrinsic(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        self.observation.check(template, meter, path)?;
        Err("unknown intrinsic")
    }
}
#[test]
fn protected_operation_callbacks_receive_actual_template_meter_and_path_and_reject_wrong_type() {
    let f = Fixture::new();
    for valid in [true, false] {
        let ty = if valid {
            value(&f)
        } else {
            SignatureTypeKey::RawPointer(Box::new(value(&f)))
        };
        let template = template(
            &f,
            vec![],
            vec![],
            vec![],
            expression(&f, DefaultExpressionKindV1::UnitLiteral, ty),
        );
        let mut resources = meter();
        let path = WirePath::root();
        let mut authority = OperationAuthority {
            observation: Observation::new(&template, &resources, &path),
            unit: value(&f),
        };
        let result =
            template.validate_operation_typing_semantics(&mut authority, &mut resources, &path);
        assert!(authority.observation.calls > 0);
        if valid {
            result.unwrap();
        } else {
            assert!(result.is_err());
        }
        assert!(resources.usage().validation_work_units >= 13);
    }
}
