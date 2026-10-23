use super::*;
use hir::ImportedCallableSource;
use scoop_identity::CallableTemplateOrigin as Origin;

impl Lowerer {
    pub(super) fn editing_imported_members(
        &mut self,
        receiver: hir::TypeId,
        members: &mut hir::EditingMembers,
    ) -> Result<(), hir::ImportedDependencyCandidateError> {
        for candidate in
            self.imported_member_candidates(receiver, hir::ImportedMemberLookup::Methods)?
        {
            if matches!(self.types[receiver], hir::Type::Interface(_))
                && !candidate.interface().type_parameters().is_empty()
            {
                continue;
            }
            if let Some(target) = callable(candidate.interface().declaration()) {
                add(
                    members,
                    candidate.name().to_owned(),
                    Target::Callable(target),
                    false,
                );
            }
        }
        for candidate in
            self.imported_member_candidates(receiver, hir::ImportedMemberLookup::PropertyGetters)?
        {
            let Origin::Accessor(accessor) = candidate.interface().declaration() else {
                unreachable!("property lookup returns accessor declarations");
            };
            let property = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.property_for_accessor(accessor))
                .expect("an accessor retains its property declaration")
                .declaration();
            add(
                members,
                candidate.name().to_owned(),
                Target::ImportedProperty(property),
                false,
            );
        }
        Ok(())
    }

    pub(super) fn editing_imported_extension_applicable(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        receiver: hir::TypeId,
    ) -> Result<bool, U> {
        if !self.imported_callable_is_accessible(declaration.interface(), Some(receiver)) {
            return Ok(false);
        }
        let signature = declaration
            .interface()
            .receiver()
            .ok_or(U::SignatureFailure)?;
        let origin =
            crate::semantic_queries::imported_location(declaration.definition_origin().origin())
                .map_err(|_| U::SignatureFailure)?;
        let parameters = self
            .query_imported_method_binders(declaration, &[], origin.span)
            .map_err(|_| U::SignatureFailure)?;
        let bindings = self.query_signature_bindings(0, &parameters);
        let declared = self
            .imported_signature_type_with_bindings(signature, &bindings)
            .map_err(|_| U::SignatureFailure)?;
        Ok(self.editing_receiver_applicable(&parameters, declared, receiver))
    }
}

pub(super) fn callable(origin: Origin) -> Option<hir::SemanticCallable> {
    Some(hir::SemanticCallable::Imported(match origin {
        Origin::Function(id) => hir::DefaultCallableDeclarationV1::Function(id),
        Origin::GenericFunction(id) => hir::DefaultCallableDeclarationV1::GenericFunction(id),
        _ => return None,
    }))
}
