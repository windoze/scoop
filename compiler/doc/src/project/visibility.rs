use super::*;
use hir::{AccessConstraint, AccessDomain, DeclaredVisibility, NominalAccess, NominalOwner};

pub(super) fn label(visibility: DeclaredVisibility) -> Visibility {
    match visibility {
        DeclaredVisibility::Public => Visibility::Public,
        DeclaredVisibility::Internal => Visibility::Internal,
        DeclaredVisibility::Private => Visibility::Private,
        DeclaredVisibility::Protected => Visibility::Protected,
    }
}

impl Projection<'_> {
    pub(super) fn is_public(&self, target: Target) -> bool {
        match target {
            Target::Declaration(source) => self.source_is_public(source),
            Target::TypeParameter { owner, .. } | Target::ContextParameter { owner, .. } => {
                self.source_is_public(owner)
            }
            Target::AnnotationParameter { annotation, .. } => {
                self.source_is_public(Source::Annotation(annotation))
            }
            Target::DerivedFunction(id) => self.domain(&self.export.functions[id].access.lookup.0),
        }
    }

    fn source_is_public(&self, target: Source) -> bool {
        let export = self.export;
        match target {
            Source::Nominal(owner) => self.domain(&self.nominal_access(owner).lookup.0),
            Source::Callable(callable) | Source::Parameter { callable, .. } => {
                self.callable_is_public(callable)
            }
            Source::Receiver(hir::SourceAnnotationReceiver::Callable(callable)) => {
                self.callable_is_public(callable)
            }
            Source::Property(id)
            | Source::Receiver(hir::SourceAnnotationReceiver::Property(id))
            | Source::PropertyStorage { property: id, .. } => {
                self.domain(&export.properties[id].access.lookup.0)
            }
            Source::Field(field) => self.domain(&export.structs[field.structure()].access.lookup.0),
            Source::ClassField(field) => {
                let owner = export.class_fields[field].owner;
                self.domain(&export.classes[owner].access.lookup.0)
            }
            Source::Variant(variant) => {
                self.domain(&export.enums[variant.enumeration()].access.lookup.0)
            }
            Source::VariantField(field) => {
                self.domain(&export.enums[field.variant().enumeration()].access.lookup.0)
            }
            Source::TypeAlias(id) => self.domain(&export.type_aliases[id].access.lookup.0),
            Source::Annotation(id) => {
                let declaration = self.declarations[&Target::Declaration(Source::Annotation(id))];
                match declaration.visibility {
                    DeclaredVisibility::Public => true,
                    DeclaredVisibility::Protected => {
                        let Some(Target::Declaration(Source::Nominal(owner))) = declaration.parent
                        else {
                            unreachable!("a protected annotation has a lexical nominal owner")
                        };
                        let hir::AnnotationTargetV1::Nominal(owner) =
                            hir::AnnotationTargetV1::from_source(export, Source::Nominal(owner))
                                .expect("the checked annotation owner has a persistent identity")
                        else {
                            unreachable!("the owner is nominal")
                        };
                        self.externally_inheritable(owner, &mut Default::default())
                    }
                    DeclaredVisibility::Internal | DeclaredVisibility::Private => false,
                }
            }
        }
    }

    fn callable_is_public(&self, callable: Callable) -> bool {
        let export = self.export;
        let domain = match callable {
            Callable::Function(id) => &export.functions[id].access.lookup.0,
            Callable::StructConstructor(id) => &export.struct_constructors[id].access.lookup.0,
            Callable::ClassConstructor(id) => &export.class_constructors[id].access.lookup.0,
            Callable::VariantConstructor(id) => &export.enums[id.enumeration()].access.lookup.0,
            Callable::Getter(id) => &export.property_getters[id].access.lookup.0,
            Callable::Setter(id) => &export.property_setters[id].access.lookup.0,
        };
        self.domain(domain)
    }

    fn domain(&self, domain: &AccessDomain) -> bool {
        self.public_domain(domain, None, &mut Default::default())
    }

    fn public_domain(
        &self,
        domain: &AccessDomain,
        owner: Option<hir::SourceNominalId>,
        active: &mut std::collections::HashSet<hir::SourceNominalId>,
    ) -> bool {
        !domain.is_empty()
            && domain.constraints().iter().all(|constraint| {
                let AccessConstraint::SubclassesOf(base) = constraint else {
                    return false;
                };
                Some(*base) == owner || self.externally_inheritable(*base, active)
            })
    }

    pub(super) fn externally_inheritable(
        &self,
        owner: hir::SourceNominalId,
        active: &mut std::collections::HashSet<hir::SourceNominalId>,
    ) -> bool {
        // The self subclass constraint is the ordinary class inheritance
        // domain. Sealed families additionally need a public open descendant.
        // A cycle through protected lexical owners supplies no external entry.
        if !active.insert(owner) {
            return false;
        }
        let id = self
            .export
            .nominal_identities
            .class_id(owner)
            .expect("protected access names a source class");
        let class = &self.export.classes[id];
        let accessible = self.public_domain(&class.access.lookup.0, None, active)
            && self.public_domain(&class.access.inheritance.0, Some(owner), active);
        let result = accessible
            && match &class.inheritance_family {
                hir::InheritanceFamily::Open => true,
                hir::InheritanceFamily::Sealed(children) => children
                    .iter()
                    .any(|child| self.externally_inheritable(*child, active)),
            };
        active.remove(&owner);
        result
    }

    fn nominal_access(&self, owner: NominalOwner) -> &NominalAccess {
        match owner {
            NominalOwner::Class(id) => &self.export.classes[id].access,
            NominalOwner::Struct(id) => &self.export.structs[id].access,
            NominalOwner::Enum(id) => &self.export.enums[id].access,
            NominalOwner::Interface(id) => &self.export.interfaces[id].access,
            NominalOwner::Object(id) => &self.export.objects[id].access,
        }
    }
}
