use crate::{CanonicalSourceParameterShapesV1, SourceParameterShapeV1};

/// A complete borrowed declaration position, not proof that the position has a
/// default. The source authority must independently join it to the queried path.
#[derive(Clone, Copy, Debug)]
pub struct DefaultTemplateProviderParameterV1<'a> {
    parameters: &'a CanonicalSourceParameterShapesV1,
    prefix: &'a [SourceParameterShapeV1],
    current: &'a SourceParameterShapeV1,
}
impl<'a> DefaultTemplateProviderParameterV1<'a> {
    pub fn try_new(
        parameters: &'a CanonicalSourceParameterShapesV1,
        position: u32,
    ) -> Result<Self, DefaultTemplateProviderParameterBuildError> {
        let current = parameters.parameters().get(position as usize).ok_or(
            DefaultTemplateProviderParameterBuildError {
                position,
                arity: parameters.len_u32(),
            },
        )?;
        Ok(Self {
            parameters,
            prefix: &parameters.parameters()[..position as usize],
            current,
        })
    }
    pub const fn parameters(self) -> &'a CanonicalSourceParameterShapesV1 {
        self.parameters
    }
    pub const fn prefix(self) -> &'a [SourceParameterShapeV1] {
        self.prefix
    }
    pub const fn current(self) -> &'a SourceParameterShapeV1 {
        self.current
    }
    pub fn position(self) -> u32 {
        self.prefix.len() as u32
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultTemplateProviderParameterBuildError {
    pub position: u32,
    pub arity: u32,
}
impl std::fmt::Display for DefaultTemplateProviderParameterBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "default provider parameter {} is outside source arity {}",
            self.position, self.arity
        )
    }
}
impl std::error::Error for DefaultTemplateProviderParameterBuildError {}
