use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::ExportDefaultReferenceOccurrenceSiteV1;
use crate::{
    CanonicalTemplateLocalTableV1, DefaultAssignTargetV1, DefaultBindingActionV1,
    DefaultBindingPlanV1, DefaultCaptureV1, DefaultCatchV1, DefaultExpressionV1,
    DefaultForIterationPlanV1, DefaultLocalFunctionV1, DefaultPatternV1, DefaultStatementV1,
    DefaultWhenArmV1, ExportDefaultBodyV1, ExportDefinitionSourceV1, TemplateLocalDefinitionV1,
    TemplateLocalRecordV1,
};

pub use super::compare::{
    CallableTargetView as DefaultCallableReferenceTargetViewV1,
    ConstructorTargetView as DefaultConstructorReferenceTargetViewV1,
    FieldTargetView as DefaultFieldReferenceTargetViewV1,
    compare_default_signature_reference_targets,
};

mod state;

/// A borrowed body reference. This view does not grant lookup or publication access.
#[derive(Clone, Copy, Debug)]
pub enum DefaultBodyReferenceTargetV1<'a> {
    Callable(DefaultCallableReferenceTargetViewV1<'a>),
    Constructor(DefaultConstructorReferenceTargetViewV1<'a>),
    Type(&'a SignatureTypeKey),
    Global(PersistentPropertyId),
    Singleton(PersistentObjectValueId),
    Field(DefaultFieldReferenceTargetViewV1<'a>),
}

/// The typed node that owns non-expression metadata. Iterator and binding plans
/// retain the source temporaries needed to replay receiver checks.
#[derive(Clone, Copy, Debug)]
pub enum DefaultBodyReferenceMetadataV1<'a> {
    TemplateLocal {
        index: usize,
        local: &'a TemplateLocalRecordV1,
    },
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Pattern(&'a DefaultPatternV1),
    Assignment(&'a DefaultAssignTargetV1),
    WhenArm(&'a DefaultWhenArmV1),
    Catch(&'a DefaultCatchV1),
    Iterator(&'a DefaultForIterationPlanV1),
    BindingPlan(&'a DefaultBindingPlanV1),
    BindingAction(&'a DefaultBindingActionV1),
    Capture(&'a DefaultCaptureV1),
    LocalFunction(&'a DefaultLocalFunctionV1),
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultBodyReferenceAttachmentV1<'a> {
    Expression {
        index: u32,
        expression: &'a DefaultExpressionV1,
    },
    Metadata(DefaultBodyReferenceMetadataV1<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct DefaultBodyReferenceOccurrenceV1<'a> {
    pub target: DefaultBodyReferenceTargetV1<'a>,
    pub definition_origin: &'a ExportDefinitionSourceV1,
    /// Legacy diagnostic category; use `attachment` to identify expression uses.
    pub site: ExportDefaultReferenceOccurrenceSiteV1,
    pub attachment: DefaultBodyReferenceAttachmentV1<'a>,
}

/// Receives the complete six-domain closure without copying signature trees.
/// Visitors independently check target, origin, access, and use coverage.
pub trait DefaultBodyReferenceVisitorV1<'body> {
    type Error: From<WireError>;

    fn expression(
        &mut self,
        index: u32,
        expression: &'body DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error>;

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error>;
}

impl ExportDefaultBodyV1 {
    /// Walks locals followed by the complete body reference closure. Expression
    /// callbacks follow wire/source preorder; reference callbacks preserve the
    /// closure traversal order. Metadata has no synthetic expression index.
    /// The walk proves no operation typing, origin, or access contract itself.
    pub fn visit_direct_references<'body, V: DefaultBodyReferenceVisitorV1<'body>>(
        &'body self,
        locals: &'body CanonicalTemplateLocalTableV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        visitor: &mut V,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), V::Error> {
        let mut walker = ReferenceWalker {
            visitor,
            meter,
            path,
            next_expression: 0,
            current: DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::Body(self),
            ),
        };
        for (index, local) in locals.records().iter().enumerate() {
            walker.enter_node(1)?;
            let origin = match local.definition() {
                TemplateLocalDefinitionV1::Source(origin) => origin,
                TemplateLocalDefinitionV1::Synthetic => definition_origin,
            };
            walker.current = DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::TemplateLocal { index, local },
            );
            walker.enter_leaf()?;
            walker.observe(
                DefaultBodyReferenceTargetV1::Type(local.value_type()),
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::TemplateLocalType { index },
            )?;
        }
        walker.current =
            DefaultBodyReferenceAttachmentV1::Metadata(DefaultBodyReferenceMetadataV1::Body(self));
        walker.walk_body(self)
    }
}

pub(super) struct ReferenceWalker<'a, 'body, V> {
    pub(super) visitor: &'a mut V,
    pub(super) meter: &'a mut BudgetMeter,
    pub(super) path: &'a WirePath,
    pub(super) current: DefaultBodyReferenceAttachmentV1<'body>,
    next_expression: u64,
}
