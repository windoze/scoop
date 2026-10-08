use super::*;
use crate::{IntrinsicTypeParameters, IntrinsicTypeTarget, NominalIntrinsicRepresentationV1};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryNominalShape {
    Reference,
    Intrinsic(NominalIntrinsicRepresentationV1),
    Struct {
        c_layout: NativeBoundaryCLayoutPolicy,
        fields: Vec<NativeBoundaryFieldDefinition>,
    },
    Enum {
        variants: Vec<NativeBoundaryVariantDefinition>,
    },
}

impl WireEncode for NativeBoundaryNominalShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Reference => encode_empty_sum(encoder, 1),
            Self::Intrinsic(representation) => encode_value_sum(encoder, 4, representation),
            Self::Struct { c_layout, fields } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                c_layout.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, fields)
            }
            Self::Enum { variants } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryTypeDefinitionRecord {
    owner: NativeBoundaryNominalOwner,
    type_parameter_count: u32,
    shape: NativeBoundaryNominalShape,
    c_abi: NativeBoundaryCAbiV1,
}

impl NativeBoundaryTypeDefinitionRecord {
    pub fn new(
        declaration: &SourceDeclarationKey,
        binder_parameter_counts: &[u32],
        shape: NativeBoundaryNominalShape,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = NativeBoundaryNominalOwner::from_declaration(declaration)
            .map_err(NativeBoundaryDefinitionError::DeclarationIdentity)?;
        let type_parameter_count = declaration.duplicate_signature().type_parameter_count();
        validate_binder_stack(type_parameter_count, binder_parameter_counts)?;
        validate_shape(declaration, owner, binder_parameter_counts, &shape)?;
        Ok(Self {
            owner,
            type_parameter_count,
            shape,
            c_abi: NativeBoundaryCAbiV1::SourceRepresentation,
        })
    }

    pub fn with_c_abi(
        mut self,
        c_abi: NativeBoundaryCAbiV1,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        c_abi.validate_shape(&self.shape)?;
        self.c_abi = c_abi;
        Ok(self)
    }

    pub const fn c_abi(&self) -> NativeBoundaryCAbiV1 {
        self.c_abi
    }

    pub const fn owner(&self) -> NativeBoundaryNominalOwner {
        self.owner
    }

    pub const fn type_parameter_count(&self) -> u32 {
        self.type_parameter_count
    }

    pub const fn shape(&self) -> &NativeBoundaryNominalShape {
        &self.shape
    }
}

impl WireEncode for NativeBoundaryTypeDefinitionRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.type_parameter_count))?;
        encoder.field(3)?;
        self.shape.encode(encoder)?;
        encoder.field(4)?;
        self.c_abi.encode(encoder)
    }
}

fn validate_shape(
    declaration: &SourceDeclarationKey,
    owner: NativeBoundaryNominalOwner,
    binder_parameter_counts: &[u32],
    shape: &NativeBoundaryNominalShape,
) -> Result<(), NativeBoundaryDefinitionError> {
    let kind_matches = match shape {
        NativeBoundaryNominalShape::Intrinsic(representation) => {
            let expected = match representation.family().target() {
                IntrinsicTypeTarget::Struct => scoop_identity::SourceDeclarationKind::Struct,
                IntrinsicTypeTarget::Class => scoop_identity::SourceDeclarationKind::Class,
            };
            declaration.declaration_kind() == expected
        }
        NativeBoundaryNominalShape::Reference => matches!(
            declaration.declaration_kind(),
            scoop_identity::SourceDeclarationKind::Class
                | scoop_identity::SourceDeclarationKind::Interface
                | scoop_identity::SourceDeclarationKind::Object
                | scoop_identity::SourceDeclarationKind::AnnotationClass
        ),
        NativeBoundaryNominalShape::Struct { .. } => {
            declaration.declaration_kind() == scoop_identity::SourceDeclarationKind::Struct
        }
        NativeBoundaryNominalShape::Enum { .. } => {
            declaration.declaration_kind() == scoop_identity::SourceDeclarationKind::Enum
        }
    };
    if !kind_matches {
        return Err(NativeBoundaryDefinitionError::ShapeKindMismatch);
    }

    let expected_owner = owner.declaration_owner();
    match shape {
        NativeBoundaryNominalShape::Reference => return Ok(()),
        NativeBoundaryNominalShape::Intrinsic(representation) => {
            let expected = match representation.family().parameters() {
                IntrinsicTypeParameters::None => 0,
                IntrinsicTypeParameters::OneInvariantUnconstrained
                | IntrinsicTypeParameters::OneInvariantRef
                | IntrinsicTypeParameters::OneInvariantValue => 1,
            };
            let actual = declaration.duplicate_signature().type_parameter_count();
            if actual != expected {
                return Err(NativeBoundaryDefinitionError::IntrinsicArityMismatch {
                    expected,
                    actual,
                });
            }
        }
        NativeBoundaryNominalShape::Struct { fields, .. } => {
            let mut seen = BTreeSet::new();
            for field in fields {
                if field.owner != expected_owner {
                    return Err(NativeBoundaryDefinitionError::FieldOwnerMismatch);
                }
                if !seen.insert(field.field) {
                    return Err(NativeBoundaryDefinitionError::DuplicateField);
                }
                validate_signature_binders(&field.ty, binder_parameter_counts)?;
            }
        }
        NativeBoundaryNominalShape::Enum { variants } => {
            let mut seen = BTreeSet::new();
            for variant in variants {
                if variant.owner != expected_owner {
                    return Err(NativeBoundaryDefinitionError::VariantOwnerMismatch);
                }
                if !seen.insert(variant.variant) {
                    return Err(NativeBoundaryDefinitionError::DuplicateVariant);
                }
                for field in &variant.fields {
                    validate_signature_binders(&field.ty, binder_parameter_counts)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_binder_stack(
    type_parameter_count: u32,
    binder_parameter_counts: &[u32],
) -> Result<(), NativeBoundaryDefinitionError> {
    let Some(&actual) = binder_parameter_counts.first() else {
        return Err(NativeBoundaryDefinitionError::EmptyBinderStack);
    };
    if actual != type_parameter_count {
        return Err(NativeBoundaryDefinitionError::BinderStackHeadMismatch {
            expected: type_parameter_count,
            actual,
        });
    }
    Ok(())
}

fn validate_signature_binders(
    ty: &SignatureTypeKey,
    binder_parameter_counts: &[u32],
) -> Result<(), NativeBoundaryDefinitionError> {
    match ty {
        SignatureTypeKey::Nominal(_) => Ok(()),
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => {
            for argument in arguments.as_slice() {
                validate_signature_binders(argument, binder_parameter_counts)?;
            }
            Ok(())
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                validate_signature_binders(parameter, binder_parameter_counts)?;
            }
            validate_signature_binders(result, binder_parameter_counts)
        }
        SignatureTypeKey::RawPointer(pointee) => {
            validate_signature_binders(pointee, binder_parameter_counts)
        }
        SignatureTypeKey::Binder { depth, index } => {
            let depth_index = usize::try_from(*depth).map_err(|_| {
                NativeBoundaryDefinitionError::BinderDepthOutOfRange { depth: *depth }
            })?;
            let Some(&count) = binder_parameter_counts.get(depth_index) else {
                return Err(NativeBoundaryDefinitionError::BinderDepthOutOfRange { depth: *depth });
            };
            if *index >= count {
                return Err(NativeBoundaryDefinitionError::BinderIndexOutOfRange {
                    depth: *depth,
                    index: *index,
                    count,
                });
            }
            Ok(())
        }
    }
}
