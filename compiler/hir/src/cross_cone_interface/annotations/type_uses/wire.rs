use super::super::parameter::DecodedAnnotationParameterTypeV1;
use super::super::wire::{
    AnnotationDataResolutionError as Error, AnnotationDataResolver, sequence,
};
use super::*;
use crate::{DecodedAnnotationTargetV1, DecodedAnnotationValueV1, TypeAnnotation, TypeUseStep};
use scoop_identity::{
    DecodedPersistentId, DecodedSourceIdentity, DecodedSourceSpan, PersistentAnnotationId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

mod encode;
mod paths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedTypeUseId {
    source: DecodedSourceIdentity,
    span: DecodedSourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedTypeAnnotation {
    annotation: DecodedPersistentId<PersistentAnnotationId>,
    arguments: Vec<DecodedAnnotationValueV1>,
    source: DecodedSourceIdentity,
    span: DecodedSourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedTypeAnnotationNode {
    path: Vec<TypeUseStep>,
    annotations: Vec<DecodedTypeAnnotation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DecodedAnnotatedTypeUseV1 {
    identity: DecodedTypeUseId,
    ty: DecodedAnnotationParameterTypeV1,
    binder_count: u32,
    nodes: Vec<DecodedTypeAnnotationNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DecodedAnnotationTypeSiteV1 {
    target: DecodedAnnotationTargetV1,
    slot: AnnotationTypeSlot,
    type_use: DecodedTypeUseId,
}

impl DecodedTypeUseId {
    fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationTypeUseId, Error<E>> {
        Ok(AnnotationTypeUseId {
            source: self.source.resolve(resolver).map_err(Error::Source)?,
            span: self.span.validate().map_err(Error::Span)?,
        })
    }
}

impl DecodedAnnotatedTypeUseV1 {
    pub(in super::super) fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotatedTypeUseV1, Error<E>> {
        Ok(AnnotatedTypeUseV1 {
            identity: self.identity.resolve(resolver)?,
            ty: self.ty.resolve(resolver).map_err(Error::Reference)?,
            binder_count: self.binder_count,
            nodes: self
                .nodes
                .into_iter()
                .map(|node| {
                    Ok(TypeAnnotationNode {
                        path: node.path,
                        annotations: node
                            .annotations
                            .into_iter()
                            .map(|application| {
                                Ok(TypeAnnotation {
                                    annotation: resolver
                                        .resolve(application.annotation)
                                        .map_err(Error::Reference)?,
                                    arguments: application
                                        .arguments
                                        .into_iter()
                                        .map(|v| v.resolve(resolver))
                                        .collect::<Result<_, _>>()?,
                                    source: application
                                        .source
                                        .resolve(resolver)
                                        .map_err(Error::Source)?,
                                    span: application.span.validate().map_err(Error::Span)?,
                                })
                            })
                            .collect::<Result<_, Error<E>>>()?,
                    })
                })
                .collect::<Result<_, Error<E>>>()?,
        })
    }
}

impl DecodedAnnotationTypeSiteV1 {
    pub(in super::super) fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationTypeSiteV1, Error<E>> {
        Ok(AnnotationTypeSiteV1 {
            target: self.target.resolve(resolver).map_err(Error::Reference)?,
            slot: self.slot,
            type_use: self.type_use.resolve(resolver)?,
        })
    }
}

impl WireDecode for DecodedTypeUseId {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        Ok(Self {
            source: d.field(1, DecodedSourceIdentity::decode)?,
            span: d.field(2, DecodedSourceSpan::decode)?,
        })
    }
}
impl WireDecode for DecodedTypeAnnotation {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            annotation: d.field(1, DecodedPersistentId::decode)?,
            arguments: d.field(2, |d| {
                d.decode_array(|d, _| DecodedAnnotationValueV1::decode(d))
            })?,
            source: d.field(3, DecodedSourceIdentity::decode)?,
            span: d.field(4, DecodedSourceSpan::decode)?,
        })
    }
}
impl WireDecode for DecodedTypeAnnotationNode {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        Ok(Self {
            path: d.field(1, |d| d.decode_array(|d, _| TypeUseStep::decode(d)))?,
            annotations: d.field(2, |d| {
                d.decode_array(|d, _| DecodedTypeAnnotation::decode(d))
            })?,
        })
    }
}
impl WireDecode for DecodedAnnotatedTypeUseV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            identity: d.field(1, DecodedTypeUseId::decode)?,
            ty: d.field(2, DecodedAnnotationParameterTypeV1::decode)?,
            binder_count: d.field(3, Decoder::u32)?,
            nodes: d.field(4, |d| {
                d.decode_array(|d, _| DecodedTypeAnnotationNode::decode(d))
            })?,
        })
    }
}
impl WireDecode for DecodedAnnotationTypeSiteV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            target: d.field(1, DecodedAnnotationTargetV1::decode)?,
            slot: d.field(2, AnnotationTypeSlot::decode)?,
            type_use: d.field(3, DecodedTypeUseId::decode)?,
        })
    }
}
