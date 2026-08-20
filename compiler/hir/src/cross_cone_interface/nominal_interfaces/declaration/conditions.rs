use super::*;
use crate::{
    BinderUseListValidationError, CanonicalBinderUseListV1, DecodedCanonicalBinderUseListV1,
    SignatureTypeReferenceResolver,
};
use scoop_identity::SignatureTypeKey;

/// Declaration conditions retained independently of any particular use or body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalInstantiationConditionsV1 {
    no_gc: bool,
    gc_free_pointees: CanonicalBinderUseListV1,
}

impl NominalInstantiationConditionsV1 {
    pub const fn new(no_gc: bool, gc_free_pointees: CanonicalBinderUseListV1) -> Self {
        Self {
            no_gc,
            gc_free_pointees,
        }
    }

    pub fn empty() -> Self {
        Self::new(
            false,
            CanonicalBinderUseListV1::try_new(Vec::new()).expect("empty binder list"),
        )
    }

    pub const fn no_gc(&self) -> bool {
        self.no_gc
    }

    pub const fn gc_free_pointees(&self) -> &CanonicalBinderUseListV1 {
        &self.gc_free_pointees
    }

    pub(in super::super) fn validate(
        &self,
        kind: PublicNominalKindV1,
        parameter_count: usize,
    ) -> Result<(), NominalInterfaceRecordBuildError> {
        if self.no_gc
            && !matches!(
                kind,
                PublicNominalKindV1::Struct | PublicNominalKindV1::Enum
            )
        {
            return Err(NominalInterfaceRecordBuildError::NoGcKind(kind));
        }
        let mut previous = None;
        for (position, argument) in self.gc_free_pointees.arguments().iter().enumerate() {
            let valid = match *argument {
                SignatureTypeKey::Binder { depth: 0, index } => {
                    let valid = (index as usize) < parameter_count
                        && previous.is_none_or(|previous| previous < index);
                    previous = Some(index);
                    valid
                }
                _ => false,
            };
            if !valid {
                return Err(NominalInterfaceRecordBuildError::PointeeConditionBinder { position });
            }
        }
        Ok(())
    }
}

impl WireEncode for NominalInstantiationConditionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.no_gc))?;
        encoder.field(2)?;
        self.gc_free_pointees.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalInstantiationConditionsV1 {
    no_gc: bool,
    gc_free_pointees: DecodedCanonicalBinderUseListV1,
}

impl DecodedNominalInstantiationConditionsV1 {
    pub fn resolve<R: SignatureTypeReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalInstantiationConditionsV1, BinderUseListValidationError<E>> {
        Ok(NominalInstantiationConditionsV1::new(
            self.no_gc,
            self.gc_free_pointees.resolve(resolver)?,
        ))
    }
}

impl WireEncode for DecodedNominalInstantiationConditionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.no_gc))?;
        encoder.field(2)?;
        self.gc_free_pointees.encode(encoder)
    }
}

impl WireDecode for DecodedNominalInstantiationConditionsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let no_gc = decoder.field(1, |decoder| match decoder.unsigned()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(WireError::new(
                scoop_wire::WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        })?;
        let gc_free_pointees = decoder.field(2, DecodedCanonicalBinderUseListV1::decode)?;
        Ok(Self {
            no_gc,
            gc_free_pointees,
        })
    }
}
