use super::ProtectedDefaultTemplateV1;
use crate::DefaultBodyValidationInputV1;

mod data_flow;
mod nested;
mod operation_typing;
pub use data_flow::ProtectedDefaultLocalDataFlowSemanticAuthority;
pub use nested::ProtectedDefaultNestedCallableSemanticAuthority;
#[cfg(test)]
mod tests;
pub use operation_typing::ProtectedDefaultOperationTypingSemanticAuthority;

impl<'a> From<&'a ProtectedDefaultTemplateV1> for DefaultBodyValidationInputV1<'a> {
    fn from(template: &'a ProtectedDefaultTemplateV1) -> Self {
        Self::new(
            template.body(),
            template.locals(),
            template.receiver(),
            template.value_parameters(),
            template.result(),
            template.allows_suspend(),
            template.definition_path(),
        )
    }
}
