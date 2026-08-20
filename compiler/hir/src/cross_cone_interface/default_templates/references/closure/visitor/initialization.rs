use super::super::walk::BodyNode;
use super::*;
use crate::{
    DefaultConstructorRefV1, DefaultFieldRefV1, ExportCommonInitializationStepV1,
    ExportConstructorDelegationV1, ExportConstructorInitializationKindV1 as Kind,
    ExportGenericNominalInitializationV1, ExportTemplateFragmentV1,
};

impl ExportTemplateFragmentV1 {
    pub fn visit_direct_references<'body, V: DefaultBodyReferenceVisitorV1<'body>>(
        &'body self,
        origin: &'body ExportDefinitionSourceV1,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), V::Error> {
        let attachment = DefaultBodyReferenceAttachmentV1::Metadata(
            DefaultBodyReferenceMetadataV1::Fragment(self),
        );
        let mut walker = ReferenceWalker {
            visitor,
            path,
            next_expression: 0,
            current: attachment,
        };
        walker.walk_locals(self.locals(), origin)?;
        walker.current = attachment;
        walker.walk_nodes(
            self.statements()
                .iter()
                .map(BodyNode::Statement)
                .chain(self.results().iter().map(BodyNode::Expression)),
        )
    }
}

impl ExportGenericNominalInitializationV1 {
    pub fn visit_direct_references<'body, V: DefaultBodyReferenceVisitorV1<'body>>(
        &'body self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), V::Error> {
        for constructor in self.constructors() {
            let origin = constructor.definition_origin();
            let attachment = DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::ConstructorInitialization(constructor),
            );
            let mut walker = ReferenceWalker {
                visitor,
                path,
                next_expression: 0,
                current: attachment,
            };
            walker.constructor(constructor.declaration(), origin)?;
            walker.walk_locals(constructor.inputs(), origin)?;
            walker.current = attachment;
            match constructor.kind() {
                Kind::StructPrimary => {}
                Kind::StructSecondary { delegation, body }
                | Kind::ClassSecondaryThis { delegation, body } => {
                    walker.delegation(delegation, origin)?;
                    body.visit_direct_references(origin, walker.visitor, path)?;
                }
                Kind::ClassPrimary {
                    base,
                    primary_stores,
                } => {
                    if let Some(base) = base {
                        walker.delegation(base, origin)?;
                    }
                    for store in primary_stores {
                        walker.field(&store.field, origin)?;
                    }
                }
                Kind::ClassSecondaryTerminal { base, body } => {
                    if let Some(base) = base {
                        walker.delegation(base, origin)?;
                    }
                    body.visit_direct_references(origin, walker.visitor, path)?;
                }
            }
        }
        if let Some(constructor) = self.constructors().first() {
            let origin = constructor.definition_origin();
            let attachment = DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::ConstructorInitialization(constructor),
            );
            let mut walker = ReferenceWalker {
                visitor,
                path,
                next_expression: 0,
                current: attachment,
            };
            for step in self.common() {
                match step {
                    ExportCommonInitializationStepV1::Field { field, value } => {
                        walker.field(field, origin)?;
                        value.visit_direct_references(origin, walker.visitor, path)?;
                    }
                    ExportCommonInitializationStepV1::Body(body) => {
                        body.visit_direct_references(origin, walker.visitor, path)?
                    }
                }
            }
        }
        if let Some(hook) = self.release_policy().hook() {
            hook.body()
                .visit_direct_references(hook.definition_origin(), visitor, path)?;
        }
        Ok(())
    }
}

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    fn constructor(
        &mut self,
        target: &'body DefaultConstructorRefV1,
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        self.walk_nodes(std::iter::once(BodyNode::ConstructorUse {
            target: DefaultConstructorReferenceTargetViewV1::Constructor(target),
            origin,
            site: ExportDefaultReferenceOccurrenceSiteV1::InitializationHeader,
        }))
    }

    fn field(
        &mut self,
        target: &'body DefaultFieldRefV1,
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        self.walk_nodes(std::iter::once(BodyNode::FieldUse {
            target: DefaultFieldReferenceTargetViewV1::Field(target),
            origin,
            site: ExportDefaultReferenceOccurrenceSiteV1::InitializationHeader,
        }))
    }

    fn delegation(
        &mut self,
        delegation: &'body ExportConstructorDelegationV1,
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        self.constructor(&delegation.target, origin)?;
        delegation
            .arguments
            .visit_direct_references(origin, self.visitor, self.path)
    }
}
