use crate::{
    EditingCallee, EditingParameter, SemanticDeclarationBinders, SemanticSourceLocation, TypeId,
    VariantStyle,
};

/// A constructor's original source calling form, without an invocation adapter.
#[derive(Clone, Debug)]
pub enum EditingConstructorForm {
    Nominal,
    Variant { name: String, style: VariantStyle },
}

/// Complete declaration types; source defaults need not have been checked yet.
#[derive(Clone, Debug)]
pub struct EditingConstructorSignature {
    pub callee: EditingCallee,
    pub form: EditingConstructorForm,
    pub binders: SemanticDeclarationBinders,
    pub parameters: Vec<EditingParameter>,
    pub result: TypeId,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Copy, Debug)]
pub enum EditingConstructorDeclaration {
    Constructor(EditingCallee),
    Parameter { callee: EditingCallee, index: usize },
}
