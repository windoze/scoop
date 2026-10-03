use scoop_wire::{WireError, WireErrorKind};

use super::super::walk::BodyNode;
use super::{
    DefaultBodyReferenceAttachmentV1 as Attachment, DefaultBodyReferenceMetadataV1 as Metadata,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1, ExportDefaultReferenceOccurrenceSiteV1,
    ExportDefinitionSourceV1, ReferenceWalker,
};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(in super::super) fn attach_node(&mut self, node: BodyNode<'body>) -> Result<(), V::Error> {
        self.current = match node {
            BodyNode::Body(body) => Attachment::Metadata(Metadata::Body(body)),
            BodyNode::Statement(statement) => Attachment::Metadata(Metadata::Statement(statement)),
            BodyNode::Expression(expression) => {
                let index = u32::try_from(self.next_expression).map_err(|_| {
                    V::Error::from(WireError::new(
                        WireErrorKind::IntegerOutOfRange,
                        self.path.clone(),
                        None,
                    ))
                })?;
                self.next_expression += 1;
                self.visitor.expression(index, expression, self.path)?;
                Attachment::Expression { index, expression }
            }
            BodyNode::Pattern { pattern, .. } => Attachment::Metadata(Metadata::Pattern(pattern)),
            BodyNode::AssignTarget { target, .. } => {
                Attachment::Metadata(Metadata::Assignment(target))
            }
            BodyNode::WhenArm(arm) => Attachment::Metadata(Metadata::WhenArm(arm)),
            BodyNode::Catch(catch) => Attachment::Metadata(Metadata::Catch(catch)),
            BodyNode::Capture(capture) => Attachment::Metadata(Metadata::Capture(capture)),
            BodyNode::LocalFunction { function, .. } => {
                Attachment::Metadata(Metadata::LocalFunction(function))
            }
            // These constituents belong to the scheduled expression or metadata plan.
            BodyNode::When { .. }
            | BodyNode::WhenGuard(_)
            | BodyNode::WhenFallback { .. }
            | BodyNode::Try(_)
            | BodyNode::Lambda { .. }
            | BodyNode::Anonymous { .. }
            | BodyNode::CallableReference { .. }
            | BodyNode::CallableUse { .. }
            | BodyNode::CallableShape { .. }
            | BodyNode::BoundCallableUse { .. }
            | BodyNode::BoundCallableShape { .. }
            | BodyNode::MethodCallee { .. }
            | BodyNode::MethodCalleeShape { .. }
            | BodyNode::ConstructorUse { .. }
            | BodyNode::VariantFieldShape { .. }
            | BodyNode::FieldUse { .. }
            | BodyNode::LiteralEquality { .. }
            | BodyNode::ArrayAssembly { .. }
            | BodyNode::IntegerArguments(_) => self.current,
        };
        Ok(())
    }

    pub(in super::super) fn observe(
        &mut self,
        target: Target<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        // A pure binder is local signature structure, not an external reference.
        if matches!(
            target,
            Target::Type(scoop_identity::SignatureTypeKey::Binder { .. })
        ) {
            return Ok(());
        }
        self.visitor.reference(
            DefaultBodyReferenceOccurrenceV1 {
                target,
                definition_origin: origin,
                site,
                attachment: self.current,
            },
            self.path,
        )
    }
}
