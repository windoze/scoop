use super::*;
use scoop_identity::{SourceDeclarationKey, SourceDeclarationKind};
use std::fmt;

mod generated;
mod members;

#[derive(Debug)]
pub enum MirTypeBridgeError {
    Reference(IdentityReferenceError),
    GeneratedRoleReference(
        Box<scoop_identity::GeneratedNominalResolutionError<IdentityReferenceError>>,
    ),
    Resource(WireError),
    ContradictoryFacts,
    ExactOriginMismatch {
        exact: PersistentExactTypeId,
    },
    InvalidSourceNominal {
        nominal: PersistentTypeId,
    },
    GeneratedRoleMismatch {
        nominal: PersistentTypeId,
    },
    MissingGeneratedFoundation {
        nominal: PersistentTypeId,
    },
    MissingShapeSupportSource {
        exact: PersistentExactTypeId,
    },
    GeneratedExecutionShapeGate {
        nominal: PersistentTypeId,
    },
    OriginRepresentationMismatch {
        exact: PersistentExactTypeId,
    },
    RepresentationFactsMismatch {
        exact: PersistentExactTypeId,
    },
    EmptyCLayout,
    FieldOwner {
        field: PersistentFieldId,
    },
    DuplicateField {
        field: PersistentFieldId,
    },
    VariantOwner {
        variant: PersistentEnumVariantId,
    },
    DuplicateVariant {
        variant: PersistentEnumVariantId,
    },
    VariantFieldOwner {
        field: PersistentEnumVariantFieldId,
    },
    DuplicateVariantField {
        field: PersistentEnumVariantFieldId,
    },
    GeneratedPayloadMismatch {
        nominal: PersistentTypeId,
    },
    InvalidBase {
        exact: PersistentExactTypeId,
    },
    InvalidInterface {
        exact: PersistentExactTypeId,
    },
    DuplicateInterface {
        exact: PersistentExactTypeId,
    },
    NonCanonicalInterfaces {
        index: usize,
    },
    DuplicateType {
        exact: PersistentExactTypeId,
    },
    NonCanonicalTypeOrder {
        index: usize,
    },
}
impl From<IdentityReferenceError> for MirTypeBridgeError {
    fn from(value: IdentityReferenceError) -> Self {
        Self::Reference(value)
    }
}
impl fmt::Display for MirTypeBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR type bridge: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeError {}

impl MirTypeBridgeAuthority<'_> {
    pub(super) fn validate(
        &self,
        record: &ParamFreeMirTypeExportV1,
    ) -> Result<(), MirTypeBridgeError> {
        let origin = record.origin();
        let exact = self
            .identities
            .canonical_key::<_, ExactTypeKey>(record.exact())?;
        if exact.as_ref() != &ExactTypeKey::Nominal(origin.nominal()) {
            return Err(MirTypeBridgeError::ExactOriginMismatch {
                exact: record.exact(),
            });
        }
        match origin {
            MirTypeOriginV1::SourceNominal(nominal) => {
                let key = self.source_nominal(*nominal)?;
                let shape_matches = matches!(
                    (key.declaration_kind(), record.representation()),
                    (
                        SourceDeclarationKind::Struct,
                        MirTypeRepresentationV1::Struct { .. }
                    ) | (
                        SourceDeclarationKind::Enum,
                        MirTypeRepresentationV1::Enum { .. }
                    ) | (
                        SourceDeclarationKind::Class,
                        MirTypeRepresentationV1::Class { .. }
                    ) | (
                        SourceDeclarationKind::Interface,
                        MirTypeRepresentationV1::Interface
                    ) | (
                        SourceDeclarationKind::Object,
                        MirTypeRepresentationV1::Object { .. }
                    ) | (
                        SourceDeclarationKind::Object | SourceDeclarationKind::Struct,
                        MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit)
                    ) | (
                        SourceDeclarationKind::Struct,
                        MirTypeRepresentationV1::Intrinsic(
                            MirParamFreeIntrinsicV1::Integer(_) | MirParamFreeIntrinsicV1::Boolean
                        )
                    ) | (
                        SourceDeclarationKind::Class,
                        MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::String)
                    )
                );
                if !shape_matches {
                    return Err(MirTypeBridgeError::OriginRepresentationMismatch {
                        exact: record.exact(),
                    });
                }
                if let MirTypeRepresentationV1::Object { backing } = record.representation() {
                    self.validate_object_backing(*nominal, *backing)?;
                }
            }
            MirTypeOriginV1::GeneratedNominal { nominal, role } => {
                self.validate_generated(*nominal, role, record)?
            }
        }
        self.validate_members(record)?;
        self.validate_facts(record)?;
        self.validate_bases(record)
    }

    fn source_nominal(
        &self,
        nominal: PersistentTypeId,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, MirTypeBridgeError> {
        let key = self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(nominal)?;
        if PersistentTypeId::from_source_declaration(&key).ok() != Some(nominal) {
            return Err(MirTypeBridgeError::InvalidSourceNominal { nominal });
        }
        Ok(key)
    }

    fn validate_facts(&self, record: &ParamFreeMirTypeExportV1) -> Result<(), MirTypeBridgeError> {
        use MirTypeRepresentationV1 as Repr;
        use MirValueKindV1 as Kind;
        let facts = record.facts();
        let valid = match record.representation() {
            Repr::Intrinsic(MirParamFreeIntrinsicV1::Unit) => facts.kind() == Kind::ZeroSizedValue,
            Repr::Intrinsic(
                MirParamFreeIntrinsicV1::Integer(_) | MirParamFreeIntrinsicV1::Boolean,
            ) => facts.kind() == Kind::NonZeroValue && facts.gc() == MirGcKindV1::GcFree,
            Repr::Intrinsic(MirParamFreeIntrinsicV1::String)
            | Repr::Class { .. }
            | Repr::Interface
            | Repr::Object { .. }
            | Repr::ObjectBacking { .. }
            | Repr::BoxedValue { .. } => facts.kind() == Kind::Reference,
            Repr::Struct {
                fields, c_layout, ..
            } => {
                if matches!(c_layout, MirTypeCLayoutPolicyV1::CLayout(_)) {
                    if fields.is_empty() {
                        return Err(MirTypeBridgeError::EmptyCLayout);
                    }
                    facts.kind() == Kind::NonZeroValue
                } else if fields.is_empty() {
                    facts.kind() == Kind::ZeroSizedValue
                } else {
                    facts.kind() != Kind::Reference
                }
            }
            Repr::Enum { variants }
            | Repr::CoroutineStep { variants }
            | Repr::CoroutineSlot { variants } => {
                let gc = if variants
                    .iter()
                    .all(|variant| variant.gc == MirGcKindV1::GcFree)
                {
                    MirGcKindV1::GcFree
                } else {
                    MirGcKindV1::ContainsManagedReferences
                };
                facts.kind() == Kind::NonZeroValue && facts.gc() == gc
            }
        };
        if !valid {
            return Err(MirTypeBridgeError::RepresentationFactsMismatch {
                exact: record.exact(),
            });
        }
        Ok(())
    }

    fn validate_bases(&self, record: &ParamFreeMirTypeExportV1) -> Result<(), MirTypeBridgeError> {
        let relation = record.base_and_interfaces();
        if let Some((index, _)) = relation
            .interfaces
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0] > pair[1])
        {
            return Err(MirTypeBridgeError::NonCanonicalInterfaces { index: index + 1 });
        }
        if let MirBaseClassV1::Base(base) = relation.base {
            if base == record.exact()
                || !matches!(
                    record.representation(),
                    MirTypeRepresentationV1::Class { .. }
                        | MirTypeRepresentationV1::Object { .. }
                        | MirTypeRepresentationV1::ObjectBacking { .. }
                )
            {
                return Err(MirTypeBridgeError::InvalidBase { exact: base });
            }
            if self.source_kind(base)? != SourceDeclarationKind::Class {
                return Err(MirTypeBridgeError::InvalidBase { exact: base });
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for interface in &relation.interfaces {
            if !seen.insert(*interface) {
                return Err(MirTypeBridgeError::DuplicateInterface { exact: *interface });
            }
            if *interface == record.exact()
                || self.source_kind(*interface)? != SourceDeclarationKind::Interface
            {
                return Err(MirTypeBridgeError::InvalidInterface { exact: *interface });
            }
        }
        Ok(())
    }

    fn source_kind(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<SourceDeclarationKind, MirTypeBridgeError> {
        let key = self.identities.canonical_key::<_, ExactTypeKey>(exact)?;
        let ExactTypeKey::Nominal(nominal) = key.as_ref() else {
            return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
        };
        Ok(self.source_nominal(*nominal)?.declaration_kind())
    }
}
