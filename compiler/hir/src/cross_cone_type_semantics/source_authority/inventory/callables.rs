//! Callable data used while assembling inheritance slot contracts.

use crate::{
    CallableModalityV1, DeclarationAccessSourceV1, InheritanceCallableDeclarationV1,
    InheritanceCallableSignatureV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSourceCallableV1 {
    declaration: InheritanceCallableDeclarationV1,
    signature: InheritanceCallableSignatureV1,
    modality: CallableModalityV1,
    declaration_access: DeclarationAccessSourceV1,
}

impl InheritanceSourceCallableV1 {
    pub const fn new(
        declaration: InheritanceCallableDeclarationV1,
        signature: InheritanceCallableSignatureV1,
        modality: CallableModalityV1,
        declaration_access: DeclarationAccessSourceV1,
    ) -> Self {
        Self {
            declaration,
            signature,
            modality,
            declaration_access,
        }
    }
    pub const fn declaration(&self) -> InheritanceCallableDeclarationV1 {
        self.declaration
    }
    pub const fn signature(&self) -> &InheritanceCallableSignatureV1 {
        &self.signature
    }
    pub const fn modality(&self) -> CallableModalityV1 {
        self.modality
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
}
