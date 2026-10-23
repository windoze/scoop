use super::*;
use hir::SourceContextConstructor as Constructor;

impl Lowerer {
    pub(in crate::editing) fn editing_construction_sources(&self) -> Vec<hir::EditingSourceBody> {
        let Some(inputs) = &self.editing_constructions else {
            return Vec::new();
        };
        let mut bodies = Vec::new();
        for input in inputs.sources.values() {
            let owner = input.owner();
            let constructors = match owner {
                Owner::Struct(id) => self.structs[id]
                    .constructors
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches!(
                            self.struct_constructors[*id].kind,
                            hir::StructConstructorKind::Secondary { .. }
                        )
                    })
                    .map(Constructor::Struct)
                    .collect::<Vec<_>>(),
                Owner::Class(id) => self.class_secondary_sources(id),
                Owner::Object(_) => Vec::new(),
                Owner::Interface(_) | Owner::Enum(_) => {
                    unreachable!("initialization inputs have constructors")
                }
            };
            for (constructor, source) in constructors.into_iter().zip(input.secondary()) {
                if let Some(delegation) = &source.delegation {
                    let span = match delegation {
                        ast::ConstructorDelegation::This { span, .. }
                        | ast::ConstructorDelegation::Super { span, .. } => *span,
                    };
                    bodies.push(self.construction_region(
                        Body::Delegation(constructor),
                        constructor,
                        input.file,
                        span,
                    ));
                }
                bodies.push(self.construction_region(
                    Body::Secondary(constructor),
                    constructor,
                    input.file,
                    source.body.span,
                ));
            }
            let Some(class) = self.construction_class(owner) else {
                continue;
            };
            let Some(constructor) = self.common_construction_context(class) else {
                continue;
            };
            if matches!(
                self.class_constructors[constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            ) && let Some(base) = inputs.bases.get(&class)
                && base.constructor_arguments().is_some()
            {
                bodies.push(self.construction_region(
                    Body::Delegation(Constructor::Class(constructor)),
                    Constructor::Class(constructor),
                    input.file,
                    base.span,
                ));
            }
            for (member, source) in input.members().iter().enumerate() {
                if let ast::ClassMember::InitBlock(init) = source {
                    bodies.push(self.construction_region(
                        Body::InitBlock {
                            owner: class,
                            member: member.try_into().expect("source member index fits u32"),
                        },
                        Constructor::Class(constructor),
                        input.file,
                        init.body.span,
                    ));
                }
            }
            if let Some(delegates) = self.interface_delegates.get(&class) {
                for delegate in delegates {
                    bodies.push(self.construction_region(
                        Body::InterfaceDelegate(delegate.field),
                        Constructor::Class(constructor),
                        input.file,
                        delegate.expression_span(),
                    ));
                }
            }
        }
        for (&property, &(file, span)) in &inputs.properties {
            let owner = self.construction_property_owner(property);
            let class = self
                .construction_class(owner)
                .expect("instance initializer has a backing class");
            if let Some(constructor) = self.common_construction_context(class) {
                bodies.push(self.construction_region(
                    Body::Property(property),
                    Constructor::Class(constructor),
                    file,
                    span,
                ));
            }
        }
        bodies
    }

    pub(in crate::editing) fn editing_construction_source(
        &self,
        id: Body,
    ) -> Option<hir::EditingSourceBody> {
        self.editing_construction_sources()
            .into_iter()
            .find(|body| body.id == hir::EditingSourceBodyId::Construction(id))
    }

    fn construction_region(
        &self,
        id: Body,
        constructor: Constructor,
        file: usize,
        span: ast::Span,
    ) -> hir::EditingSourceBody {
        let owner = match constructor {
            Constructor::Class(id) => match self
                .object_by_backing_class
                .get(&self.class_constructors[id].owner)
            {
                Some(object) => hir::EditingSourceBodyOwner::Initialization(
                    self.singleton_values[self.objects[*object].singleton_value].initialization,
                ),
                None => hir::EditingSourceBodyOwner::Constructor(
                    hir::SemanticConstructor::CurrentClass(id),
                ),
            },
            Constructor::Struct(id) => hir::EditingSourceBodyOwner::Constructor(
                hir::SemanticConstructor::CurrentStruct(id),
            ),
        };
        hir::EditingSourceBody {
            id: hir::EditingSourceBodyId::Construction(id),
            owner,
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span,
            },
        }
    }

    pub(super) fn construction_body_owner(&self, body: Body) -> Owner {
        match body {
            Body::Delegation(Constructor::Class(id)) | Body::Secondary(Constructor::Class(id)) => {
                self.class_construction_owner(self.class_constructors[id].owner)
            }
            Body::Delegation(Constructor::Struct(id))
            | Body::Secondary(Constructor::Struct(id)) => {
                Owner::Struct(self.struct_constructors[id].owner)
            }
            Body::Property(id) => self.construction_property_owner(id),
            Body::InterfaceDelegate(field) => {
                self.class_construction_owner(self.class_fields[field].owner)
            }
            Body::InitBlock { owner, .. } => self.class_construction_owner(owner),
        }
    }

    pub(super) fn class_construction_owner(&self, class: hir::ClassId) -> Owner {
        self.object_by_backing_class
            .get(&class)
            .map_or(Owner::Class(class), |object| Owner::Object(*object))
    }

    fn construction_property_owner(&self, property: hir::PropertyId) -> Owner {
        match self.properties[property].owner {
            hir::PropertyOwner::Class(class) => Owner::Class(class),
            hir::PropertyOwner::Object(object) => Owner::Object(object),
            _ => unreachable!("construction property source belongs to a class or object"),
        }
    }

    pub(super) fn construction_class(&self, owner: Owner) -> Option<hir::ClassId> {
        match owner {
            Owner::Class(class) => Some(class),
            Owner::Object(object) => Some(self.objects[object].backing_class),
            _ => None,
        }
    }

    pub(super) fn common_construction_context(
        &self,
        class: hir::ClassId,
    ) -> Option<hir::ClassConstructorId> {
        self.classes[class]
            .constructors
            .iter()
            .copied()
            .find(|id| {
                matches!(
                    self.class_constructors[*id].kind,
                    hir::ClassConstructorKind::Primary { .. }
                )
            })
            .or_else(|| self.classes[class].constructors.first().copied())
    }

    pub(super) fn class_secondary_sources(&self, class: hir::ClassId) -> Vec<Constructor> {
        self.classes[class]
            .constructors
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    self.class_constructors[*id].kind,
                    hir::ClassConstructorKind::Secondary { .. }
                )
            })
            .map(Constructor::Class)
            .collect()
    }
}
