use super::*;
use crate::aliases::{AliasOrigin, AliasTemplate};
use crate::call_resolution::candidates::NominalConstructorSource as Source;
use crate::editing::types::TypeTarget;
use crate::imports::lookup::TypeLookupTarget;

impl Lowerer {
    pub(crate) fn observe_nominal_constructor_name(
        &mut self,
        span: ast::Span,
        source: Source,
        alias: Option<&AliasTemplate>,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let owner = match source {
            Source::Class(constructor) => {
                crate::Owner::Class(self.class_constructors[constructor].owner)
            }
            Source::Struct(constructor) => {
                crate::Owner::Struct(self.struct_constructors[constructor].owner)
            }
            Source::IntrinsicClass(class)
            | Source::ArrayGenerate(class)
            | Source::Atomic(class) => crate::Owner::Class(class),
            Source::ImportedArray(owner)
            | Source::ImportedArrayGenerate(owner)
            | Source::ImportedAtomic(owner) => {
                self.observe_constructor_name(span, owner, alias);
                return;
            }
            Source::Variant(_) => return,
        };
        self.observe_constructor_name(span, self.nominal_identity(owner).declaration_id(), alias);
    }

    pub(crate) fn observe_dependency_constructor_name(
        &mut self,
        span: ast::Span,
        declaration: CallableTemplateOrigin,
        owner: hir::PublicDeclarationOwnerV1,
        alias: Option<&AliasTemplate>,
    ) {
        if !self.observes_current_body()
            || !matches!(declaration, CallableTemplateOrigin::Constructor(_))
        {
            return;
        }
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = owner else {
            unreachable!("a selected constructor retains its nominal owner")
        };
        self.observe_constructor_name(span, owner, alias);
    }

    fn observe_constructor_name(
        &mut self,
        span: ast::Span,
        owner: hir::SourceNominalId,
        alias: Option<&AliasTemplate>,
    ) {
        let Some(span) = self.editing_construction_type_name(span) else {
            return;
        };
        match alias {
            Some(AliasTemplate {
                origin: AliasOrigin::Source(alias),
                ..
            }) => self.observe_type_name(span, TypeTarget::Alias(*alias)),
            Some(AliasTemplate {
                origin: AliasOrigin::Imported(binding),
                name,
                ..
            }) => self.observe_type_lookup(
                &ast::Ident {
                    text: name.text.clone(),
                    span,
                },
                &TypeLookupTarget::Dependency(binding.as_ref().clone()),
                None,
            ),
            None => self.observe_type_name(span, TypeTarget::Nominal(owner)),
        }
    }
}
