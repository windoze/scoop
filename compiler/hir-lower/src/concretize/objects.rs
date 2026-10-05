//! Object declarations and values are allocated when their representation is needed.

use super::initialization::InitializationSource;
use super::*;
mod imported;

impl Concretizer<'_> {
    pub(super) fn register_object(
        &mut self,
        source_id: export::ObjectId,
        backing_class: concrete::ClassId,
        canonical_type: concrete::TypeId,
    ) {
        let source = &self.source.objects[source_id];
        let source_value = &self.source.singleton_values[source.singleton_value];
        let object = concrete::ObjectId::from_raw(
            u32::try_from(self.objects.len())
                .expect("objects fit their typed id domain")
                .into(),
        );
        let object_type = self.object_types.alloc(concrete::ObjectType {
            declaration: object,
            representation: backing_class,
            canonical_type,
        });
        assert!(
            self.object_type_map
                .insert(backing_class, object_type)
                .is_none()
        );
        let value = concrete::SingletonValueId::from_raw(
            u32::try_from(self.singleton_values.len())
                .expect("singleton values fit their typed id domain")
                .into(),
        );
        let published_root =
            self.singleton_published_roots
                .alloc(concrete::SingletonPublishedRoot {
                    value,
                    ty: canonical_type,
                });
        assert!(
            self.singleton_root_map
                .insert(value, published_root)
                .is_none()
        );
        let initialization = self.request_initialization(InitializationKey {
            source: InitializationSource::Defined(source_value.initialization),
            arguments: self.classes[backing_class].type_arguments.clone(),
        });
        let allocated_value = self.singleton_values.alloc(concrete::SingletonValue {
            identity: self.source.object_value_identities[source.singleton_value].id(),
            declaration: object,
            object_type,
            published_root,
            initialization,
        });
        assert_eq!(allocated_value, value);
        assert!(
            self.singleton_value_map
                .insert(object_type, value)
                .is_none()
        );
        let kind = match source.kind {
            export::ObjectKind::Standalone => concrete::ObjectKind::Standalone,
            export::ObjectKind::Companion(relation) => {
                let relation = &self.source.companion_relations[relation];
                let host = self
                    .lower_nominal_owner(Some(relation.host))
                    .expect("a companion relation has a nominal host");
                let relation = self.companion_relations.alloc(concrete::CompanionRelation {
                    host,
                    object,
                    name: match &relation.name {
                        export::CompanionName::Default => concrete::CompanionName::Default,
                        export::CompanionName::Named(name) => {
                            concrete::CompanionName::Named(name.clone())
                        }
                    },
                });
                concrete::ObjectKind::Companion(relation)
            }
        };
        let allocated_object = self.objects.alloc(concrete::ObjectDecl {
            origin: self.source.nominal_identities[source_id].clone(),
            name: source.name.clone(),
            owner: self.lower_nominal_owner(source.owner),
            object_type,
            singleton_value: value,
            kind,
            backing_class,
            span: source.span,
        });
        assert_eq!(allocated_object, object);
    }

    pub(super) fn lower_object_type(
        &mut self,
        source: export::ObjectTypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ObjectTypeId {
        let representation = self.source.object_types[source].representation;
        let class = self.lower_class_application(representation, substitution);
        self.object_type_map[&class]
    }

    pub(super) fn singleton_value_for_type(
        &self,
        ty: concrete::TypeId,
    ) -> concrete::SingletonValueId {
        let concrete::TypeKind::Class(class) = self.types[ty].kind else {
            unreachable!("a singleton has a class representation")
        };
        self.singleton_value_map[&self.object_type_map[&class]]
    }

    pub(super) fn singleton_value_for_application(
        &self,
        source: export::SingletonValueId,
        arguments: &[concrete::TypeId],
    ) -> concrete::SingletonValueId {
        let object = &self.source.objects[self.source.singleton_values[source].declaration];
        let owner = self.source.nominal_identities[object.backing_class].declaration_id();
        let class = self.class_by_key[&(owner, arguments.to_vec())];
        self.singleton_value_map[&self.object_type_map[&class]]
    }

    pub(super) fn lower_singleton_value(
        &mut self,
        source: export::SingletonValueId,
        arguments: &[concrete::TypeId],
    ) -> concrete::SingletonValueId {
        let value = &self.source.singleton_values[source];
        let object_type = self.lower_object_type(value.object_type, arguments);
        self.singleton_value_map[&object_type]
    }

    pub(super) fn lower_singleton_root(
        &mut self,
        source: export::SingletonPublishedRootId,
        substitution: &[concrete::TypeId],
    ) -> concrete::SingletonPublishedRootId {
        let value = self.lower_singleton_value(
            self.source.singleton_published_roots[source].value,
            substitution,
        );
        self.singleton_root_map[&value]
    }
}
