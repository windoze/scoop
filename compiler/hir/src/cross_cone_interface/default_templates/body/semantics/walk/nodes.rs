use super::*;

#[derive(Clone, Copy)]
pub(in super::super) enum WorkItem<'a> {
    Body {
        node: BodyNode<'a>,
    },
    Type {
        signature: &'a SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
}

#[derive(Clone, Copy)]
pub(in super::super) enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    Pattern {
        pattern: &'a DefaultPatternV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    AssignTarget {
        target: &'a DefaultAssignTargetV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    When {
        value: &'a DefaultWhenV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    WhenArm(&'a DefaultWhenArmV1),
    WhenGuard {
        guard: &'a DefaultWhenGuardV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    WhenFallback {
        fallback: &'a DefaultWhenFallbackV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Try {
        value: &'a DefaultTryV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Catch(&'a DefaultCatchV1),
    For {
        plan: &'a DefaultForIterationPlanV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingPlan {
        plan: &'a DefaultBindingPlanV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingAction(&'a DefaultBindingActionV1),
    BindingShape {
        shape: &'a DefaultBindingShapeV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingTemporary {
        temporary: &'a DefaultBindingTemporaryV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingLeaf {
        leaf: &'a DefaultBindingLeafV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingProjection {
        projection: &'a DefaultBindingProjectionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    IteratorConformance(&'a DefaultIteratorConformanceV1),
    IteratorNext(&'a DefaultIteratorNextV1),
    AppliedOption {
        option: &'a DefaultAppliedOptionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    LocalFunction {
        function: &'a DefaultLocalFunctionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Lambda {
        lambda: &'a DefaultLambdaV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    AnonymousFunction {
        function: &'a DefaultAnonymousFunctionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    CallableReference {
        reference: &'a DefaultCallableReferenceV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Capture(&'a DefaultCaptureV1),
    CallableRef {
        callable: &'a DefaultCallableRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableRef {
        callable: &'a DefaultBoundCallableRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableSource {
        source: &'a DefaultBoundCallableSourceV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    MethodCallee {
        callee: &'a DefaultMethodCalleeV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    ConstructorRef {
        constructor: &'a DefaultConstructorRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    EnumVariantRef {
        variant: &'a DefaultEnumVariantRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    EnumVariantFieldRef {
        field: &'a DefaultEnumVariantFieldRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    FieldRef {
        field: &'a DefaultFieldRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    LiteralEquality {
        equality: &'a DefaultLiteralEqualityV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    ArrayAssembly {
        assembly: &'a DefaultArrayAssemblyV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    IntegerArguments(&'a DefaultIntegerArgumentsV1),
    Origin {
        source: &'a ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,
    },
}
