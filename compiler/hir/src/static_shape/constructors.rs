use super::*;
use scoop_identity::CallableTemplateOrigin;

mod parameters;
pub use parameters::*;

#[derive(Clone, Copy)]
enum ParameterSource<'a> {
    Current(&'a ExportParameterInterface),
    Dependency {
        parameters: &'a CallableSourceInterfaceV1,
        interface: &'a CrossConeHirInterfaceSectionV1,
    },
}

#[derive(Clone, Copy)]
enum ParameterTargets<'a> {
    Struct,
    Variant(StaticVariantShape<'a>),
    Class(&'a ClassConstructor),
    ImportedClass(&'a ClassPrimaryConstructorV1),
}

#[derive(Clone, Copy)]
pub struct StaticConstructorShape<'a> {
    owner: StaticNominalShape<'a>,
    identity: CallableTemplateOrigin,
    source: ParameterSource<'a>,
    targets: ParameterTargets<'a>,
}

impl<'a> StaticConstructorShape<'a> {
    pub const fn identity(self) -> CallableTemplateOrigin {
        self.identity
    }

    pub fn parameters(self) -> impl ExactSizeIterator<Item = StaticConstructorParameter<'a>> {
        let len = match self.source {
            ParameterSource::Current(source) => source.parameters.len(),
            ParameterSource::Dependency { parameters, .. } => {
                parameters.parameters().parameters().len()
            }
        };
        (0..len).map(move |index| StaticConstructorParameter {
            constructor: self,
            index,
        })
    }

    fn current(
        owner: StaticNominalShape<'a>,
        identity: CallableTemplateOrigin,
        parameter_owner: ExportParameterOwner,
        targets: ParameterTargets<'a>,
    ) -> Self {
        let source = owner
            .module
            .source_parameter_interfaces
            .iter()
            .find(|source| source.owner == parameter_owner)
            .expect("a source constructor retains its parameter interface");
        Self {
            owner,
            identity,
            source: ParameterSource::Current(source),
            targets,
        }
    }

    fn dependency(
        owner: StaticNominalShape<'a>,
        identity: CallableTemplateOrigin,
        interface: &'a CrossConeHirInterfaceSectionV1,
        targets: ParameterTargets<'a>,
    ) -> Self {
        let parameters = interface
            .source_interfaces()
            .get(identity)
            .expect("a published source constructor retains its parameter interface");
        Self {
            owner,
            identity,
            source: ParameterSource::Dependency {
                parameters,
                interface,
            },
            targets,
        }
    }
}

impl<'a> StaticNominalShape<'a> {
    pub fn primary_constructor(
        self,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> Option<StaticConstructorShape<'a>> {
        if !matches!(
            self.kind(),
            StaticNominalKind::Struct | StaticNominalKind::Class
        ) {
            return None;
        }
        match self.origin {
            StaticNominalOrigin::Current(NominalOwner::Struct(structure)) => {
                let id = self.module.structs[structure]
                    .constructors
                    .iter()
                    .copied()
                    .find(|id| {
                        matches!(
                            self.module.struct_constructors[*id].kind,
                            StructConstructorKind::Primary
                        )
                    })?;
                Some(StaticConstructorShape::current(
                    self,
                    CallableTemplateOrigin::Constructor(
                        self.module.constructor_identities[id].id(),
                    ),
                    ExportParameterOwner::StructConstructor(id),
                    ParameterTargets::Struct,
                ))
            }
            StaticNominalOrigin::Current(NominalOwner::Class(class)) => {
                let id = self.module.classes[class]
                    .constructors
                    .iter()
                    .copied()
                    .find(|id| {
                        let value = &self.module.class_constructors[*id];
                        value.identity_kind == ClassConstructorIdentityKind::Source
                            && matches!(value.kind, ClassConstructorKind::Primary { .. })
                    })?;
                let identity = self.module.constructor_identities[id]
                    .source_record()
                    .expect("the selected primary constructor is a source declaration")
                    .id();
                Some(StaticConstructorShape::current(
                    self,
                    CallableTemplateOrigin::Constructor(identity),
                    ExportParameterOwner::ClassConstructor(id),
                    ParameterTargets::Class(&self.module.class_constructors[id]),
                ))
            }
            StaticNominalOrigin::Dependency(owner) => {
                let provider = dependencies
                    .nominal_source_provider(owner)
                    .expect("a loaded nominal retains its source provider");
                let interface = provider.source_interface();
                let nominal = interface
                    .nominal_interfaces()
                    .declaration(owner)
                    .expect("the provider was selected by this nominal");
                let details = nominal.declaration_details();
                let (identity, targets) = match self.kind() {
                    StaticNominalKind::Struct => (
                        details.primary_value_constructor()?,
                        ParameterTargets::Struct,
                    ),
                    StaticNominalKind::Class => {
                        let primary = details.class_primary_constructor()?;
                        (
                            primary.constructor(),
                            ParameterTargets::ImportedClass(primary),
                        )
                    }
                    _ => {
                        unreachable!("only ordinary structs and classes have primary constructors")
                    }
                };
                Some(StaticConstructorShape::dependency(
                    self,
                    CallableTemplateOrigin::Constructor(identity),
                    interface,
                    targets,
                ))
            }
            _ => unreachable!("only ordinary structs and classes have primary constructors"),
        }
    }
}

impl<'a> StaticVariantShape<'a> {
    pub fn constructor(
        self,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> StaticConstructorShape<'a> {
        let identity = CallableTemplateOrigin::VariantConstructor(self.identity());
        let targets = ParameterTargets::Variant(self);
        match self.owner.origin {
            StaticNominalOrigin::Current(NominalOwner::Enum(enumeration)) => {
                let variant = EnumVariantRef::checked(
                    &self.owner.module.enums,
                    enumeration,
                    self.index as u32,
                )
                .expect("the view iterates this enum's variants");
                StaticConstructorShape::current(
                    self.owner,
                    identity,
                    ExportParameterOwner::VariantConstructor(variant),
                    targets,
                )
            }
            StaticNominalOrigin::Dependency(owner) => {
                let provider = dependencies
                    .nominal_source_provider(owner)
                    .expect("a loaded enum retains its source provider");
                StaticConstructorShape::dependency(
                    self.owner,
                    identity,
                    provider.source_interface(),
                    targets,
                )
            }
            _ => unreachable!("a variant belongs to an enum"),
        }
    }
}
