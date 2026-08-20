use scoop_wire::{WireError, WirePath};

use super::{ExportGenericCallableBodyV1, GenericCallableBodyBuildError as Error};
use crate::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceMetadataV1,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultBodyReferenceVisitorV1,
    DefaultCallableReferenceTargetViewV1, DefaultCaptureSourceV1, DefaultCaptureV1,
    DefaultExpressionKindV1, DefaultExpressionV1,
};

pub(super) fn validate(body: &ExportGenericCallableBodyV1) -> Result<(), Error> {
    body.visit_direct_references(
        &mut CaptureInputs {
            count: body.capture_types().len(),
        },
        &WirePath::root(),
    )
}

struct CaptureInputs {
    count: usize,
}

impl CaptureInputs {
    fn require(&self, index: u32) -> Result<(), Error> {
        if usize::try_from(index).is_ok_and(|index| index < self.count) {
            Ok(())
        } else {
            Err(Error::CaptureIndex {
                index,
                count: self.count,
            })
        }
    }

    fn sources(&self, captures: &[DefaultCaptureV1]) -> Result<(), Error> {
        for capture in captures {
            if let DefaultCaptureSourceV1::EnclosingCapture(index) = capture.source() {
                self.require(*index)?;
            }
        }
        Ok(())
    }
}

impl<'body> DefaultBodyReferenceVisitorV1<'body> for CaptureInputs {
    type Error = Error;

    fn expression(
        &mut self,
        _: u32,
        expression: &'body DefaultExpressionV1,
        _: &WirePath,
    ) -> Result<(), Error> {
        match expression.kind() {
            DefaultExpressionKindV1::Capture(index) => self.require(*index),
            DefaultExpressionKindV1::Lambda(lambda) => self.sources(lambda.captures()),
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.sources(function.captures())
            }
            DefaultExpressionKindV1::CallableReference(reference) => {
                self.sources(reference.captures())
            }
            _ => Ok(()),
        }
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        _: &WirePath,
    ) -> Result<(), Error> {
        if let DefaultBodyReferenceTargetV1::Callable(
            DefaultCallableReferenceTargetViewV1::LocalFunction(_),
        ) = occurrence.target
            && let DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::LocalFunction(function),
            ) = occurrence.attachment
        {
            self.sources(function.captures())?;
        }
        Ok(())
    }
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::CaptureReferences(error)
    }
}
