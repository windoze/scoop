use crate::{FunctionId, Span, TypeId};

/// Position in an ordered requirement list, independent of value parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContextParameterIndex(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContextParameterLabel {
    Named(String),
    Unnamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextParameter {
    pub label: ContextParameterLabel,
    pub ty: TypeId,
    pub span: Span,
}

/// A dependency member contract expressed in its nominal owner's binder domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedContextContract {
    pub declaration: crate::DefaultCallableDeclarationV1,
    pub name: String,
    pub parameters: Vec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextRequirementOwner {
    Source(FunctionId),
    Imported(crate::DefaultCallableDeclarationV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContextDiagnostic {
    pub declaration: String,
    pub label: ContextParameterLabel,
}

impl scoop_wire::WireEncode for ContextDiagnostic {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text(&self.declaration)?;
        encoder.field(2)?;
        match &self.label {
            ContextParameterLabel::Named(name) => encoder.text(name),
            ContextParameterLabel::Unnamed => encoder.text("_"),
        }
    }
}

impl scoop_wire::WireDecode for ContextDiagnostic {
    fn decode(decoder: &mut scoop_wire::Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        decoder.expect_map(2)?;
        let declaration = decoder.field(1, |decoder| decoder.text().map(str::to_owned))?;
        let label = decoder.field(2, |decoder| decoder.text().map(str::to_owned))?;
        let label = if label == "_" {
            ContextParameterLabel::Unnamed
        } else {
            ContextParameterLabel::Named(label)
        };
        Ok(Self { declaration, label })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextRequirementRef {
    pub declaration: ContextRequirementOwner,
    pub parameter: ContextParameterIndex,
    pub diagnostic: ContextDiagnostic,
}
