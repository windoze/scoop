//! Type projection uses declaration binders independently of lexical access.

use crate::*;

#[derive(Clone, Copy, Debug)]
pub enum SemanticTypeContext<'a> {
    Lexical(SemanticContext),
    Declaration(&'a SemanticDeclarationBinders),
}

impl From<SemanticContext> for SemanticTypeContext<'_> {
    fn from(context: SemanticContext) -> Self {
        Self::Lexical(context)
    }
}

impl<'a> From<&'a SemanticDeclarationBinders> for SemanticTypeContext<'a> {
    fn from(binders: &'a SemanticDeclarationBinders) -> Self {
        Self::Declaration(binders)
    }
}

/// Actual parameters returned by declaration queries, in source binder order.
/// This describes types; it does not select a lexical or access context.
#[derive(Clone, Debug)]
pub struct SemanticDeclarationBinders {
    pub owner: Vec<TypeParamDecl>,
    pub callable: Vec<TypeParamDecl>,
}

impl SemanticDeclarationBinders {
    pub fn parameters(&self) -> impl Iterator<Item = &TypeParamDecl> {
        self.owner.iter().chain(&self.callable)
    }

    pub fn signature_binders(&self) -> Vec<HirSignatureBinder> {
        self.owner
            .iter()
            .enumerate()
            .map(|(index, parameter)| (u32::from(!self.callable.is_empty()), index, parameter))
            .chain(
                self.callable
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| (0, index, parameter)),
            )
            .map(|(depth, index, parameter)| HirSignatureBinder {
                parameter: parameter.id,
                depth,
                index: u32::try_from(index).expect("declaration parameter arity fits u32"),
            })
            .collect()
    }
}

impl SemanticNominalDescription {
    pub fn declaration_binders(&self) -> SemanticDeclarationBinders {
        SemanticDeclarationBinders {
            owner: self.parameters.clone(),
            callable: Vec::new(),
        }
    }
}

impl SemanticMethodDescription {
    pub fn declaration_binders(&self) -> SemanticDeclarationBinders {
        SemanticDeclarationBinders {
            owner: self.owner_parameters.clone(),
            callable: self
                .type_parameters
                .iter()
                .map(|parameter| parameter.declaration.clone())
                .collect(),
        }
    }
}

impl<R, P, F> SemanticFunctionSignature<R, P, F> {
    pub fn declaration_binders(&self) -> SemanticDeclarationBinders {
        let (owner, callable) = self.type_parameters.split_at(self.owner_parameter_count);
        SemanticDeclarationBinders {
            owner: owner.to_vec(),
            callable: callable.to_vec(),
        }
    }
}
