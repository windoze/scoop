use super::*;

mod parents;
mod primitives;

#[derive(Clone, Copy)]
pub enum StaticNominalOrigin {
    Current(NominalOwner),
    Dependency(SourceNominalId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticNominalKind {
    Struct,
    Enum,
    Class,
    Object,
    Interface,
    Intrinsic(IntrinsicTypeKind),
}

#[derive(Clone, Copy)]
pub(super) enum Definition<'a> {
    Struct(&'a StructDefinition),
    Enum(&'a EnumDefinition),
    Class(&'a ClassDefinition),
    Interface(&'a InterfaceDefinition),
    Primitive(IntrinsicTypeKind),
}

#[derive(Clone, Copy)]
pub struct StaticNominalShape<'a> {
    pub(super) module: &'a Module,
    pub(super) ty: TypeId,
    pub(super) declaration: SourceNominalId,
    pub(super) arguments: &'a [TypeId],
    pub(super) origin: StaticNominalOrigin,
    pub(super) definition: Definition<'a>,
    kind: StaticNominalKind,
}

impl<'a> StaticNominalShape<'a> {
    pub(super) fn new(module: &'a Module, ty: TypeId) -> Self {
        let (declaration, arguments, definition) = match module.types[ty] {
            Type::Struct(application) => {
                let application = &module.struct_applications[application];
                (
                    application.template,
                    application.arguments.as_slice(),
                    Definition::Struct(module.struct_definition(application.template)),
                )
            }
            Type::Enum(application) => {
                let application = &module.enum_applications[application];
                (
                    application.template,
                    application.arguments.as_slice(),
                    Definition::Enum(module.enum_definition(application.template)),
                )
            }
            Type::Class(application) => {
                let application = &module.class_applications[application];
                (
                    application.template,
                    application.arguments.as_slice(),
                    Definition::Class(module.class_definition(application.template)),
                )
            }
            Type::Interface(application) => {
                let application = &module.interface_applications[application];
                (
                    application.template,
                    application.arguments.as_slice(),
                    Definition::Interface(module.interface_definition(application.template)),
                )
            }
            Type::Integer(kind) => {
                return Self::primitive(module, ty, IntrinsicTypeKind::Integer(kind));
            }
            Type::Boolean => return Self::primitive(module, ty, IntrinsicTypeKind::Boolean),
            Type::String => return Self::primitive(module, ty, IntrinsicTypeKind::String),
            _ => unreachable!("static nominal queries select a nominal type"),
        };
        let mut origin = module
            .nominal_identities
            .declaration(declaration)
            .map(StaticNominalOrigin::Current)
            .unwrap_or(StaticNominalOrigin::Dependency(declaration));
        let mut declaration = declaration;
        if let StaticNominalOrigin::Current(NominalOwner::Class(class)) = origin
            && let Some((object, _)) = module
                .objects
                .iter()
                .find(|(_, object)| object.backing_class == class)
        {
            origin = StaticNominalOrigin::Current(NominalOwner::Object(object));
            declaration = module.nominal_identities[object].declaration_id();
        }
        let kind = match definition {
            Definition::Struct(definition) => match definition.representation {
                StructRepresentation::Declared(_) => StaticNominalKind::Struct,
                StructRepresentation::Intrinsic(kind) => StaticNominalKind::Intrinsic(kind),
            },
            Definition::Enum(_) => StaticNominalKind::Enum,
            Definition::Class(definition) => match definition.representation {
                ClassRepresentation::Intrinsic(kind) => StaticNominalKind::Intrinsic(kind),
                ClassRepresentation::Declared => match origin {
                    StaticNominalOrigin::Current(NominalOwner::Object(_)) => {
                        StaticNominalKind::Object
                    }
                    StaticNominalOrigin::Dependency(owner)
                        if module.loaded_class_definitions[&owner]
                            .declaration
                            .interface
                            .kind()
                            == PublicNominalKindV1::Object =>
                    {
                        StaticNominalKind::Object
                    }
                    _ => StaticNominalKind::Class,
                },
            },
            Definition::Interface(_) => StaticNominalKind::Interface,
            Definition::Primitive(kind) => StaticNominalKind::Intrinsic(kind),
        };
        Self {
            module,
            ty,
            declaration,
            arguments,
            origin,
            definition,
            kind,
        }
    }

    pub const fn declaration(self) -> SourceNominalId {
        self.declaration
    }
    pub const fn origin(self) -> StaticNominalOrigin {
        self.origin
    }
    pub const fn kind(self) -> StaticNominalKind {
        self.kind
    }
    pub const fn application_type(self) -> TypeId {
        self.ty
    }
    pub const fn arguments(self) -> &'a [TypeId] {
        self.arguments
    }

    pub fn parameters(self) -> &'a [TypeParamDecl] {
        match self.definition {
            Definition::Struct(value) => &value.type_params,
            Definition::Enum(value) => &value.type_params,
            Definition::Class(value) => &value.type_params,
            Definition::Interface(value) => &value.type_params,
            Definition::Primitive(_) => &[],
        }
    }

    pub fn type_use(self, ty: TypeId) -> StaticShapeTypeUse<'a> {
        StaticShapeTypeUse {
            source: StaticShapeTypeSource::Hir(ty),
            parameters: self.parameters(),
            arguments: self.arguments,
        }
    }
}
