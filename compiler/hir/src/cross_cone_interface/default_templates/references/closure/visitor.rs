use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use super::ExportDefaultReferenceOccurrenceSiteV1;
use crate::{
    CanonicalTemplateLocalTableV1, DefaultAssignTargetV1, DefaultBindingActionV1,
    DefaultBindingPlanV1, DefaultCaptureV1, DefaultCatchV1, DefaultExpressionV1,
    DefaultForIterationPlanV1, DefaultLocalFunctionV1, DefaultPatternV1, DefaultStatementV1,
    DefaultWhenArmV1, ExportDefaultBodyV1, ExportDefinitionSourceV1, ExportGenericCallableBodyV1,
    TemplateLocalDefinitionV1, TemplateLocalRecordV1,
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
    CallableBody(&'a ExportGenericCallableBodyV1),
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

        path: &WirePath,
    ) -> Result<(), Self::Error>;

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,

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

        path: &WirePath,
    ) -> Result<(), V::Error> {
        let mut walker = ReferenceWalker {
            visitor,

            path,
            next_expression: 0,
            current: DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::Body(self),
            ),
        };
        walker.walk_locals(locals, definition_origin)?;
        walker.current =
            DefaultBodyReferenceAttachmentV1::Metadata(DefaultBodyReferenceMetadataV1::Body(self));
        walker.walk_body(self)
    }
}

impl ExportGenericCallableBodyV1 {
    /// Visits the same typed node references as source defaults, while retaining
    /// a callable implementation root and its explicit return statements.
    pub fn visit_direct_references<'body, V: DefaultBodyReferenceVisitorV1<'body>>(
        &'body self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), V::Error> {
        let attachment = DefaultBodyReferenceAttachmentV1::Metadata(
            DefaultBodyReferenceMetadataV1::CallableBody(self),
        );
        let mut walker = ReferenceWalker {
            visitor,
            path,
            next_expression: 0,
            current: attachment,
        };
        walker.observe(
            DefaultBodyReferenceTargetV1::Type(self.result()),
            self.definition_origin(),
            ExportDefaultReferenceOccurrenceSiteV1::CallableBodyHeader { field: 5, index: 0 },
        )?;
        for (index, capture_type) in self.capture_types().iter().enumerate() {
            walker.observe(
                DefaultBodyReferenceTargetV1::Type(capture_type),
                self.definition_origin(),
                ExportDefaultReferenceOccurrenceSiteV1::CallableBodyHeader { field: 10, index },
            )?;
        }
        walker.walk_locals(self.locals(), self.definition_origin())?;
        walker.current = attachment;
        walker.walk_statements(self.statements())
    }
}

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    fn walk_locals(
        &mut self,
        locals: &'body CanonicalTemplateLocalTableV1,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        for (index, local) in locals.records().iter().enumerate() {
            let origin = match local.definition() {
                TemplateLocalDefinitionV1::Source(origin) => origin,
                TemplateLocalDefinitionV1::Synthetic => definition_origin,
            };
            self.current = DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::TemplateLocal { index, local },
            );
            self.observe(
                DefaultBodyReferenceTargetV1::Type(local.value_type()),
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::TemplateLocalType { index },
            )?;
        }
        Ok(())
    }
}

pub(super) struct ReferenceWalker<'a, 'body, V> {
    pub(super) visitor: &'a mut V,

    pub(super) path: &'a WirePath,
    pub(super) current: DefaultBodyReferenceAttachmentV1<'body>,
    next_expression: u64,
}
