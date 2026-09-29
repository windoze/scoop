use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::DefaultBodyReferenceAttachmentV1;
use crate::{
    DefaultAnonymousFunctionV1, DefaultArrayAssemblyV1, DefaultAssignTargetV1,
    DefaultBoundCallableRefV1, DefaultCallableRefV1, DefaultCallableReferenceV1, DefaultCaptureV1,
    DefaultCatchV1, DefaultExpressionV1, DefaultIntegerArgumentsV1, DefaultLambdaV1,
    DefaultLiteralEqualityV1, DefaultLocalFunctionV1, DefaultMethodCalleeV1, DefaultPatternV1,
    DefaultStatementV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1, DefaultWhenGuardV1,
    DefaultWhenV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
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
    GenericDelegate {
        target: &'a crate::DefaultGenericDelegateReferenceV1,
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
    IntegerArguments(&'a DefaultIntegerArgumentsV1),
}
