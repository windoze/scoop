use super::*;
use DefaultSourceNestedCallableDescriptorV1 as Descriptor;
use scoop_wire::WireErrorKind;

impl<'a, K> DefaultSourceNestedCallablesV1<'a, K> {
    fn push(
        &mut self,
        descriptor: Descriptor<'a>,
        definition_origin: &'a ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), WireError> {
        let ordinal = u64::try_from(self.occurrences.len())
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;

        scoop_wire::allocation::try_reserve_count(&mut self.occurrences, 1, path)?;
        self.occurrences
            .push(DefaultSourceNestedCallableOccurrenceV1 {
                site: DefaultNestedCallableSiteV1::Body { ordinal },
                descriptor,
                definition_origin,
            });
        Ok(())
    }
}
impl<'a, K> DefaultBodyReferenceVisitorV1<'a> for DefaultSourceNestedCallablesV1<'a, K> {
    type Error = WireError;

    fn expression(
        &mut self,
        _: u32,
        expression: &'a DefaultExpressionV1,

        path: &WirePath,
    ) -> Result<(), WireError> {
        let descriptor = match expression.kind() {
            DefaultExpressionKindV1::Lambda(f) => Descriptor::Lambda(f),
            DefaultExpressionKindV1::AnonymousFunction(f) => Descriptor::AnonymousFunction(f),
            DefaultExpressionKindV1::CallableReference(f) => Descriptor::CallableReference(f),
            // Ordinary expressions contain no descriptor of their own.
            _ => return Ok(()),
        };
        self.push(descriptor, expression.definition_origin(), path)
    }
    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'a>,

        path: &WirePath,
    ) -> Result<(), WireError> {
        // A local-call use is an expression; only declaration metadata owns a descriptor.
        if let (
            DefaultBodyReferenceTargetV1::Callable(
                DefaultCallableReferenceTargetViewV1::LocalFunction(_),
            ),
            DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::LocalFunction(function),
            ),
        ) = (occurrence.target, occurrence.attachment)
        {
            self.push(
                Descriptor::LocalFunction(function),
                occurrence.definition_origin,
                path,
            )?;
        }
        Ok(())
    }
}
