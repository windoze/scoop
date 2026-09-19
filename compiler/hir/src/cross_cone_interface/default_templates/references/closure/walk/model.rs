use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::DefaultBodyReferenceAttachmentV1;
use crate::{
    DefaultAnonymousFunctionV1, DefaultAppliedOptionV1, DefaultArrayAssemblyV1,
    DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBoundCallableRefV1,
    DefaultCallableRefV1, DefaultCallableReferenceV1, DefaultCaptureV1, DefaultCatchV1,
    DefaultExpressionV1, DefaultForIterationPlanV1, DefaultIntegerArgumentsV1,
    DefaultIntegerOperationV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    DefaultLambdaV1, DefaultLiteralEqualityV1, DefaultLocalFunctionV1, DefaultMethodCalleeV1,
    DefaultPatternV1, DefaultStatementV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenGuardV1, DefaultWhenV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
};
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
#[derive(Clone, Copy)]
pub(super) struct ScheduledWork<'a> {
    pub(super) work: WorkItem<'a>,
    pub(super) attachment: DefaultBodyReferenceAttachmentV1<'a>,
}

#[derive(Clone, Copy)]
pub(super) enum WorkItem<'a> {
    Body {
        node: BodyNode<'a>,
        depth: u64,
    },
    Type {
        target: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Callable {
        target: CallableTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Constructor {
        target: ConstructorTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Global {
        target: PersistentPropertyId,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Singleton {
        target: PersistentObjectValueId,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Field {
        target: FieldTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
}

#[derive(Clone, Copy)]
pub(in super::super) enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    Pattern {
        pattern: &'a DefaultPatternV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    AssignTarget {
        target: &'a DefaultAssignTargetV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    When {
        value: &'a DefaultWhenV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    WhenArm(&'a DefaultWhenArmV1),
    WhenGuard(&'a DefaultWhenGuardV1),
    WhenFallback {
        fallback: &'a DefaultWhenFallbackV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Try(&'a DefaultTryV1),
    Catch(&'a DefaultCatchV1),
    For {
        plan: &'a DefaultForIterationPlanV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingPlan {
        plan: &'a DefaultBindingPlanV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingAction(&'a DefaultBindingActionV1),
    BindingShape {
        shape: &'a DefaultBindingShapeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingProjection {
        projection: &'a DefaultBindingProjectionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingTemporary {
        value_type: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingLeaf {
        value_type: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
    },
    IteratorConformance(&'a DefaultIteratorConformanceV1),
    IteratorNext(&'a DefaultIteratorNextV1),
    AppliedOption {
        option: &'a DefaultAppliedOptionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    LocalFunction {
        function: &'a DefaultLocalFunctionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Lambda {
        lambda: &'a DefaultLambdaV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Anonymous {
        function: &'a DefaultAnonymousFunctionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    CallableReference {
        reference: &'a DefaultCallableReferenceV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Capture(&'a DefaultCaptureV1),
    CallableUse {
        callable: &'a DefaultCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    CallableShape {
        callable: &'a DefaultCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableUse {
        callable: &'a DefaultBoundCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableShape {
        callable: &'a DefaultBoundCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    MethodCallee {
        callee: &'a DefaultMethodCalleeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    MethodCalleeShape {
        callee: &'a DefaultMethodCalleeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    ConstructorUse {
        target: ConstructorTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    VariantFieldShape {
        field: &'a crate::DefaultEnumVariantFieldRefV1,
        origin: &'a ExportDefinitionSourceV1,
        site: crate::DefaultBodyProviderTypeSiteV1,
    },
    FieldUse {
        target: FieldTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    LiteralEquality {
        equality: &'a DefaultLiteralEqualityV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    ArrayAssembly {
        assembly: &'a DefaultArrayAssemblyV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    IntegerOperation {
        operation: &'a DefaultIntegerOperationV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    IntegerArguments(&'a DefaultIntegerArgumentsV1),
}
