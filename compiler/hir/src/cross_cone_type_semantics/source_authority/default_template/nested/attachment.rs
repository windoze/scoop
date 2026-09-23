//! Selects actual source nodes before checking candidate identities.
use super::*;
use DefaultSourceNestedCallableDescriptorV1 as Descriptor;
use DefaultSourceNestedCallableQueryError as Error;

impl<'a, K: Copy> DefaultSourceNestedCallablesV1<'a, K> {
    /// Membership supplies no occurrence-dependent ABI or capture facts.
    pub fn require_local_declaration(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        meter.charge_work((self.occurrences.len() as u64).saturating_mul(65), path)?;
        let identity = DefaultNestedCallableIdentityV1::LocalFunction(declaration);
        if self
            .occurrences
            .iter()
            .any(|o| o.descriptor.identity() == identity)
        {
            Ok(())
        } else {
            Err(Error::MissingIdentity(identity))
        }
    }

    /// The same invoke identity can occur at independently substituted nodes.
    /// Select by the actual borrowed node, never by candidate identity alone.
    pub fn attached_to(
        &self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'a>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&DefaultSourceNestedCallableOccurrenceV1<'a>, Error> {
        let descriptor = match occurrence.attachment {
            DefaultBodyReferenceAttachmentV1::Expression { expression, .. } => {
                match expression.kind() {
                    DefaultExpressionKindV1::Lambda(f) => Descriptor::Lambda(f),
                    DefaultExpressionKindV1::AnonymousFunction(f) => {
                        Descriptor::AnonymousFunction(f)
                    }
                    DefaultExpressionKindV1::CallableReference(f) => {
                        Descriptor::CallableReference(f)
                    }
                    _ => return Err(Error::Attachment),
                }
            }
            DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::LocalFunction(f),
            ) => Descriptor::LocalFunction(f),
            _ => return Err(Error::Attachment),
        };
        meter.charge_work((self.occurrences.len() as u64).saturating_mul(65), path)?;
        let source = self
            .occurrences
            .iter()
            .find(|o| same_node(o.descriptor, descriptor))
            .ok_or(Error::MissingIdentity(descriptor.identity()))?;
        self.lookup(source.site(), descriptor.identity(), meter, path)
    }
}

fn same_node(left: Descriptor<'_>, right: Descriptor<'_>) -> bool {
    match (left, right) {
        (Descriptor::LocalFunction(a), Descriptor::LocalFunction(b)) => std::ptr::eq(a, b),
        (Descriptor::Lambda(a), Descriptor::Lambda(b)) => std::ptr::eq(a, b),
        (Descriptor::AnonymousFunction(a), Descriptor::AnonymousFunction(b)) => std::ptr::eq(a, b),
        (Descriptor::CallableReference(a), Descriptor::CallableReference(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}
