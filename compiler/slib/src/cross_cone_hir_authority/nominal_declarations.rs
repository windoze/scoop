use super::*;
use scoop_hir::{DeclaredVisibilityV1, NestedSourceMemberRefV1, NominalInterfaceRecordV1};
use scoop_identity::DefinitionOriginSubject;

type Error = CrossConeHirNominalAuthorityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn validate_shared_nominal_declaration(
        &mut self,
        record: &NominalInterfaceRecordV1,
    ) -> Result<(), Error> {
        let owner = record.declaration();
        let details = record.declaration_details();
        let key = self.source_nominal_key(owner)?;
        let subject = match owner {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        };
        let origin = self
            .current_foundation
            .definition_origin(subject)
            .ok_or(Error::MissingDefinitionOrigin { subject })?;
        if origin.origin().source().cone() != self.current
            || key
                .scope()
                .source()
                .is_some_and(|source| source != origin.origin().source())
        {
            return Err(invalid(
                owner,
                "definition source differs from its typed declaration",
            ));
        }

        let public = self
            .current_interface
            .nominal_interfaces()
            .get(owner)
            .is_some();
        for (index, atom) in key.owners().owners().iter().enumerate() {
            let parent = nominal_atom(atom)
                .ok_or_else(|| invalid(owner, "lexical parent is not a source nominal"))?;
            let parent_key = self.source_nominal_key(parent)?;
            if parent_key.origin() != self.current
                || parent_key.owners().owners() != &key.owners().owners()[..index]
            {
                return Err(invalid(
                    owner,
                    "lexical parent has a different typed owner chain",
                ));
            }
            let parent_record = self
                .current_interface
                .nominal_interfaces()
                .declaration(parent)
                .ok_or(Error::MissingNominalInterface {
                    origin: self.current,
                    declaration: parent,
                })?;
            if public
                && parent_record.declaration_details().declared_visibility()
                    != DeclaredVisibilityV1::Public
            {
                return Err(invalid(
                    owner,
                    "public lookup escapes a restricted lexical parent",
                ));
            }
        }
        if details.declared_visibility() == DeclaredVisibilityV1::Protected {
            let Some(atom) = key.owners().owners().last() else {
                return Err(invalid(
                    owner,
                    "protected declaration has no lexical class owner",
                ));
            };
            let parent =
                nominal_atom(atom).ok_or_else(|| invalid(owner, "invalid protected owner"))?;
            if self.source_nominal_key(parent)?.declaration_kind()
                != scoop_identity::SourceDeclarationKind::Class
            {
                return Err(invalid(
                    owner,
                    "protected declaration requires a class owner",
                ));
            }
        }
        let expected = PublicDeclarationOwnerV1::Nominal(owner);
        for id in details.constructors().values() {
            if self.constructor_owner(*id)? != expected {
                return Err(invalid(
                    owner,
                    "declared constructor has a different typed owner",
                ));
            }
        }
        for member in details.members().values() {
            if self.member_owner(public_member(*member))? != expected {
                return Err(invalid(
                    owner,
                    "declared member has a different typed owner",
                ));
            }
        }
        for child in details.children().values() {
            let child_key = self.source_nominal_key(*child)?;
            self.require_current("declared child", child_key.origin())?;
            if self.source_key_owner("declared child", &child_key)? != expected {
                return Err(invalid(owner, "declared child has a different typed owner"));
            }
        }
        for member in record.members().members() {
            if let PublicMemberRefV1::Callable(CallableTemplateOrigin::Accessor(id)) = member {
                let accessor = self
                    .identities
                    .canonical_key::<PersistentPropertyAccessorId, PropertyAccessorKey>(*id)
                    .map_err(Error::Identity)?;
                let PropertyOwner::Property(property) = accessor.owner() else {
                    return Err(invalid(owner, "extension accessor is not a nominal member"));
                };
                if !details
                    .members()
                    .values()
                    .contains(&NestedSourceMemberRefV1::Property(property))
                {
                    return Err(invalid(
                        owner,
                        "public accessor has no declared logical property",
                    ));
                }
            }
        }
        for binding in record.nested_bindings().values() {
            let binding = self
                .identities
                .canonical_key::<PersistentExportBindingId, ExportBindingKey>(*binding)
                .map_err(Error::Identity)?;
            let child = match binding.target() {
                BindableEntity::Type(id) => SourceNominalId::Concrete(id),
                BindableEntity::GenericType(id) => SourceNominalId::GenericTemplate(id),
                BindableEntity::ObjectValue(id) => {
                    let key = self
                        .identities
                        .canonical_key::<PersistentObjectValueId, SourceDeclarationKey>(id)
                        .map_err(Error::Identity)?;
                    SourceNominalId::from_source_declaration(&key)
                        .map_err(|_| invalid(owner, "nested object value has no source nominal"))?
                }
                _ => {
                    return Err(invalid(
                        owner,
                        "nested type binding has a non-nominal target",
                    ));
                }
            };
            if !details.children().values().contains(&child) {
                return Err(invalid(
                    owner,
                    "public nested binding has no declared child",
                ));
            }
        }
        Ok(())
    }
}

fn nominal_atom(atom: &DefinitionOwnerAtom) -> Option<SourceNominalId> {
    match atom {
        DefinitionOwnerAtom::Type(id) => Some(SourceNominalId::Concrete(*id)),
        DefinitionOwnerAtom::GenericType(id) => Some(SourceNominalId::GenericTemplate(*id)),
        _ => None,
    }
}

fn public_member(member: NestedSourceMemberRefV1) -> PublicMemberRefV1 {
    match member {
        NestedSourceMemberRefV1::Function(id) => {
            PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(id))
        }
        NestedSourceMemberRefV1::GenericFunction(id) => {
            PublicMemberRefV1::Callable(CallableTemplateOrigin::GenericFunction(id))
        }
        NestedSourceMemberRefV1::Property(id) => {
            PublicMemberRefV1::Property(PropertyOwner::Property(id))
        }
    }
}

fn invalid(declaration: SourceNominalId, reason: &'static str) -> Error {
    Error::NominalDeclaration {
        declaration,
        reason,
    }
}
