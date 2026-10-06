use super::*;
use scoop_identity::{
    CoreBuiltinNominal, DeclarationName, DefinitionOwnerAtom, Effect, SourceDeclarationKey,
};

impl<'a> ShapeDump<'a, '_> {
    pub(super) fn type_name(&self, ty: &SignatureTypeKey, parameters: &[TypeParamDecl]) -> String {
        match ty {
            SignatureTypeKey::Nominal(id) => self.nominal_name(SourceNominalId::Concrete(*id)),
            SignatureTypeKey::NominalApplication { origin, arguments } => format!(
                "{}<{}>",
                self.nominal_name(SourceNominalId::GenericTemplate(*origin)),
                self.type_list(arguments.as_slice(), parameters)
            ),
            SignatureTypeKey::Tuple(elements) => {
                format!("({})", self.type_list(elements.as_slice(), parameters))
            }
            SignatureTypeKey::Function {
                effect,
                parameters: arguments,
                result,
            } => format!(
                "{}({}) -> {}",
                if *effect == Effect::Suspend {
                    "suspend "
                } else {
                    ""
                },
                self.type_list(arguments, parameters),
                self.type_name(result, parameters)
            ),
            SignatureTypeKey::RawPointer(pointee) => {
                format!("Ptr<{}>", self.type_name(pointee, parameters))
            }
            SignatureTypeKey::NativeFunctionPointer {
                parameters: arguments,
                result,
                ..
            } => format!(
                "FunPtr<({}) -> {}>",
                self.type_list(arguments, parameters),
                self.type_name(result, parameters)
            ),
            SignatureTypeKey::Binder { depth, index } => {
                assert_eq!(*depth, 0, "a nominal source shape has one parameter frame");
                parameters[*index as usize].name.clone()
            }
        }
    }

    fn type_list(&self, types: &[SignatureTypeKey], parameters: &[TypeParamDecl]) -> String {
        types
            .iter()
            .map(|ty| self.type_name(ty, parameters))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn nominal_name(&self, owner: SourceNominalId) -> String {
        let declaration = self.declaration(owner);
        let DeclarationName::Named(name) = declaration.name() else {
            unreachable!("source nominal has a name")
        };
        let parent = declaration
            .owners()
            .owners()
            .last()
            .map(|owner| match owner {
                DefinitionOwnerAtom::Type(id) => SourceNominalId::Concrete(*id),
                DefinitionOwnerAtom::GenericType(id) => SourceNominalId::GenericTemplate(*id),
                _ => unreachable!("source nominal declarations have nominal lexical owners"),
            });
        parent.map_or_else(
            || name.as_str().to_owned(),
            |parent| format!("{}.{}", self.nominal_name(parent), name.as_str()),
        )
    }

    fn declaration(&self, owner: SourceNominalId) -> &'a SourceDeclarationKey {
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            let record = self.module.nominal_identities.core_builtin(builtin);
            if owner == SourceNominalId::Concrete(record.id()) {
                return record.key();
            }
        }
        if let Some(local) = self.module.nominal_identities.declaration(owner) {
            let identity = match local {
                NominalOwner::Struct(id) => &self.module.nominal_identities[id],
                NominalOwner::Enum(id) => &self.module.nominal_identities[id],
                NominalOwner::Class(id) => &self.module.nominal_identities[id],
                NominalOwner::Interface(id) => &self.module.nominal_identities[id],
                NominalOwner::Object(id) => &self.module.nominal_identities[id],
            };
            return identity
                .source()
                .expect("signature selected a source declaration")
                .declaration();
        }
        let provider = self
            .world
            .nominal_source_provider(owner)
            .expect("source signature retains its provider");
        let foundation = provider.source_foundation();
        match owner {
            SourceNominalId::Concrete(id) => foundation
                .source_type_key(id)
                .expect("source nominal retains its declaration key"),
            SourceNominalId::GenericTemplate(id) => {
                foundation
                    .generic_type_by_bytes(id.as_array())
                    .expect("generic nominal retains its declaration key")
                    .1
            }
        }
    }
}
