use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use super::{DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1};
use crate::{DefaultCaptureIndexError, IndexedDefaultCaptureV1, TemplateLocalIndexResolver};

use super::super::{DefaultExpressionIndexError, IndexedDefaultExpressionV1};

#[derive(Debug)]
pub struct IndexedDefaultCallableReferenceV1<'a> {
    reference: &'a DefaultCallableReferenceV1,
    target: IndexedDefaultCallableReferenceTargetV1<'a>,
    captures: Vec<IndexedDefaultCaptureV1<'a>>,
}

#[derive(Debug)]
enum IndexedDefaultCallableReferenceTargetV1<'a> {
    Named {
        callee: &'a crate::DefaultCallableRefV1,
    },
    Local {
        declaration: &'a scoop_identity::CallableTemplateOrigin,
        callee: &'a crate::DefaultCallableRefV1,
    },
    BoundMember {
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        callee: &'a crate::DefaultMethodCalleeV1,
    },
    BoundExtension {
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        callee: &'a crate::DefaultCallableRefV1,
    },
}

impl DefaultCallableReferenceV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultCallableReferenceV1<'_>, DefaultCallableReferenceIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let target = match &self.target {
            DefaultCallableReferenceTargetV1::Named(callee) => {
                IndexedDefaultCallableReferenceTargetV1::Named { callee }
            }
            DefaultCallableReferenceTargetV1::Local {
                declaration,
                callee,
            } => IndexedDefaultCallableReferenceTargetV1::Local {
                declaration,
                callee,
            },
            DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                IndexedDefaultCallableReferenceTargetV1::BoundMember {
                    receiver: Box::new(receiver.index_locals(resolver).map_err(|error| {
                        DefaultCallableReferenceIndexError::TargetReceiver {
                            target_tag: 3,
                            error: Box::new(error),
                        }
                    })?),
                    callee,
                }
            }
            DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                IndexedDefaultCallableReferenceTargetV1::BoundExtension {
                    receiver: Box::new(receiver.index_locals(resolver).map_err(|error| {
                        DefaultCallableReferenceIndexError::TargetReceiver {
                            target_tag: 4,
                            error: Box::new(error),
                        }
                    })?),
                    callee,
                }
            }
        };
        let mut captures = Vec::with_capacity(self.capture_count as usize);
        for (index, capture) in self.captures.iter().enumerate() {
            captures.push(
                capture.index_local(resolver).map_err(|error| {
                    DefaultCallableReferenceIndexError::Capture { index, error }
                })?,
            );
        }
        Ok(IndexedDefaultCallableReferenceV1 {
            reference: self,
            target,
            captures,
        })
    }
}

impl WireEncode for IndexedDefaultCallableReferenceV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.reference.invoke.encode(encoder)?;
        encoder.field(2)?;
        self.reference.definition_path.encode(encoder)?;
        encoder.field(3)?;
        self.target.encode(encoder)?;
        encoder.field(4)?;
        self.reference.function_type.encode(encoder)?;
        encoder.field(5)?;
        encode_sequence(encoder, &self.captures)?;
        encoder.field(6)?;
        encoder.unsigned(u64::from(self.reference.owner_type_parameter_count))
    }
}

impl WireEncode for IndexedDefaultCallableReferenceTargetV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named { callee } => encode_single_payload(encoder, 1, *callee),
            Self::Local {
                declaration,
                callee,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                declaration.encode(encoder)?;
                encoder.field(2)?;
                callee.encode(encoder)
            }
            Self::BoundMember { receiver, callee } => {
                encode_bound_target(encoder, 3, receiver.as_ref(), *callee)
            }
            Self::BoundExtension { receiver, callee } => {
                encode_bound_target(encoder, 4, receiver.as_ref(), *callee)
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCallableReferenceIndexError<E> {
    TargetReceiver {
        target_tag: u64,
        error: Box<DefaultExpressionIndexError<E>>,
    },
    Capture {
        index: usize,
        error: DefaultCaptureIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultCallableReferenceIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetReceiver { target_tag, error } => write!(
                formatter,
                "cannot index default callable-reference target {target_tag} receiver: {error}"
            ),
            Self::Capture { index, error } => write!(
                formatter,
                "cannot index default callable-reference capture {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultCallableReferenceIndexError<E> {}

fn encode_bound_target(
    encoder: &mut Encoder,
    tag: u64,
    receiver: &impl WireEncode,
    callee: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    receiver.encode(encoder)?;
    encoder.field(2)?;
    callee.encode(encoder)
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}
