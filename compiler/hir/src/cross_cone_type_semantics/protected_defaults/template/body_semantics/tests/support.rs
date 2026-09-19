use super::*;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn value(f: &Fixture) -> SignatureTypeKey {
    SignatureTypeKey::Nominal(f.type_id)
}
pub(super) fn path(role: StructuralDefinitionSiteRole, index: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [StructuralPathSegment::new(role, index)],
    )
}
pub(super) fn expression(
    f: &Fixture,
    kind: DefaultExpressionKindV1,
    ty: SignatureTypeKey,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, ty, f.origin()).unwrap()
}
pub(super) fn unit(f: &Fixture) -> DefaultExpressionV1 {
    expression(f, DefaultExpressionKindV1::UnitLiteral, value(f))
}
pub(super) fn statement(f: &Fixture, kind: DefaultStatementKindV1) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, f.origin()).unwrap()
}
pub(super) fn local(f: &Fixture, selector: LocalValueSelector) -> TemplateLocalRecordV1 {
    let definition = if matches!(selector, LocalValueSelector::Synthetic { .. }) {
        TemplateLocalDefinitionV1::Synthetic
    } else {
        TemplateLocalDefinitionV1::Source(f.origin())
    };
    TemplateLocalRecordV1::try_new(selector, value(f), CanonicalBooleanV1::False, definition)
        .unwrap()
}
pub(super) fn template(
    f: &Fixture,
    locals: Vec<TemplateLocalRecordV1>,
    parameters: Vec<TemplateValueParameterV1>,
    statements: Vec<DefaultStatementV1>,
    result: DefaultExpressionV1,
) -> ProtectedDefaultTemplateV1 {
    let ty = result.result_type().clone();
    ProtectedDefaultTemplateV1::try_new(
        ProtectedDefaultTemplateKeyV1::try_new(
            CallableTemplateOrigin::Function(f.function),
            parameters.len() as u32,
        )
        .unwrap(),
        PersistentLexicalRootV1::Function(f.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(statements, result).unwrap(),
        ty,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(parameters).unwrap(),
        ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
            .unwrap(),
        f.origin(),
    )
    .unwrap()
}
pub(super) struct Observation<'a> {
    pub template: &'a ProtectedDefaultTemplateV1,
    meter: *const BudgetMeter,
    path: &'a WirePath,
    pub calls: usize,
}
impl<'a> Observation<'a> {
    pub fn new(
        template: &'a ProtectedDefaultTemplateV1,
        meter: &BudgetMeter,
        path: &'a WirePath,
    ) -> Self {
        Self {
            template,
            meter,
            path,
            calls: 0,
        }
    }
    pub fn check(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        assert!(std::ptr::eq(template, self.template));
        assert!(std::ptr::eq(meter, self.meter));
        assert!(std::ptr::eq(path, self.path));
        self.calls += 1;
        meter.charge_work(13, path).map_err(|_| "callback budget")
    }
}
