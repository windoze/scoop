use super::*;
use scoop_identity::SignatureTypeKey;

impl<'a> StaticNominalShape<'a> {
    pub fn base_class(self, world: &ImportedSemanticWorld<'a>) -> Option<StaticShapeTypeUse<'a>> {
        match self.definition {
            Definition::Class(value) => value.base_class.map(|ty| self.type_use(ty)),
            Definition::Primitive(_) => self
                .published_parents(world)
                .iter()
                .find(|ty| parent_kind(ty, world) == PublicNominalKindV1::Class)
                .map(|ty| self.published_type(ty)),
            _ => None,
        }
    }

    pub fn interfaces(
        self,
        world: &ImportedSemanticWorld<'a>,
    ) -> impl ExactSizeIterator<Item = StaticShapeTypeUse<'a>> {
        let types = match self.definition {
            Definition::Struct(value) => value.interfaces.as_slice(),
            Definition::Enum(value) => value.interfaces.as_slice(),
            Definition::Class(value) => value.interfaces.as_slice(),
            Definition::Interface(value) => value.parents.as_slice(),
            Definition::Primitive(_) => {
                return self
                    .published_parents(world)
                    .iter()
                    .filter(|ty| parent_kind(ty, world) == PublicNominalKindV1::Interface)
                    .map(|ty| self.published_type(ty))
                    .collect::<Vec<_>>()
                    .into_iter();
            }
        };
        types
            .iter()
            .map(move |ty| self.type_use(*ty))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn published_type(self, signature: &'a SignatureTypeKey) -> StaticShapeTypeUse<'a> {
        StaticShapeTypeUse {
            source: StaticShapeTypeSource::Published(signature),
            parameters: self.parameters(),
            arguments: self.arguments,
        }
    }

    fn published_parents(self, world: &ImportedSemanticWorld<'a>) -> &'a [SignatureTypeKey] {
        world
            .nominal_source_provider(self.declaration)
            .expect("a primitive retains its core source provider")
            .source_interface()
            .nominal_interfaces()
            .declaration(self.declaration)
            .expect("the provider was selected by this primitive")
            .exact_supertypes()
            .values()
    }
}

fn parent_kind(ty: &SignatureTypeKey, world: &ImportedSemanticWorld<'_>) -> PublicNominalKindV1 {
    let declaration = match ty {
        SignatureTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
        SignatureTypeKey::NominalApplication { origin, .. } => {
            SourceNominalId::GenericTemplate(*origin)
        }
        _ => unreachable!("checked source supertypes are nominal applications"),
    };
    world
        .nominal_source_provider(declaration)
        .expect("a source supertype retains its provider")
        .source_interface()
        .nominal_interfaces()
        .declaration(declaration)
        .expect("the provider was selected by this supertype")
        .kind()
}
