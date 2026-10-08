//! Value boxing retains source representations and external implementations.

use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn ensure_box_source(&mut self, ty: concrete::TypeId) {
        let (family, application) = match self.types[ty].kind {
            concrete::TypeKind::Ptr(pointee) => {
                self.ensure_pointer_source(pointee);
                return;
            }
            concrete::TypeKind::Tuple(_) => {
                self.shared_types.insert(ty);
                return;
            }
            concrete::TypeKind::Unit => (
                export::IntrinsicTypeKind::Unit,
                concrete::IntrinsicTypeRepresentation::Unit,
            ),
            concrete::TypeKind::Integer(kind) => (
                export::IntrinsicTypeKind::Integer(kind),
                concrete::IntrinsicTypeRepresentation::Integer(kind),
            ),
            concrete::TypeKind::Boolean => (
                export::IntrinsicTypeKind::Boolean,
                concrete::IntrinsicTypeRepresentation::Boolean,
            ),
            _ => return,
        };
        if self
            .structs
            .iter()
            .any(|(_, declaration)| declaration.canonical_type == ty)
        {
            return;
        }
        if let export::CoreProtocols::Defined(core) = self.core {
            let owner = match family {
                export::IntrinsicTypeKind::Unit => core.fundamental_types.unit,
                export::IntrinsicTypeKind::Integer(kind) => {
                    core.fundamental_types.integers.owner(kind)
                }
                export::IntrinsicTypeKind::Boolean => core.fundamental_types.boolean,
                _ => unreachable!("primitive boxing selects a fixed value representation"),
            };
            self.lower_struct_application(self.source.structs[owner].self_application, &[]);
            return;
        }
        let source = self
            .source
            .imported_intrinsic_types
            .get(&family)
            .expect("boxed primitive HIR retains its complete dependency declaration");
        let key = (source.declaration.owner(), Vec::new());
        if self.struct_by_key.contains_key(&key) {
            return;
        }
        let definition = ResolvedStructDefinition::from_intrinsic(source, family, application);
        let id = self.allocate_struct_definition(&definition, Vec::new());
        self.complete_struct_definition(id, definition, &[]);
    }

    pub(in crate::concretize) fn ensure_pointer_source(
        &mut self,
        pointee: concrete::TypeId,
    ) -> concrete::StructId {
        let owner = match self.core {
            export::CoreProtocols::Defined(core) => {
                self.source.nominal_identities[core.ffi.ptr].declaration_id()
            }
            export::CoreProtocols::Imported(core) => export::SourceNominalId::GenericTemplate(
                core.fundamental_types().ptr().persistent(),
            ),
        };
        self.ensure_struct_definition(
            owner,
            vec![pointee],
            ConcreteApplicationRepresentation::Intrinsic(
                concrete::IntrinsicTypeRepresentation::Ptr { pointee },
            ),
        )
    }

    pub(in crate::concretize) fn ensure_coercion_box_sources(
        &mut self,
        source: concrete::TypeId,
        target: concrete::TypeId,
    ) {
        if source == target {
            return;
        }
        if let (concrete::TypeKind::Function(source), concrete::TypeKind::Function(target)) =
            (&self.types[source].kind, &self.types[target].kind)
        {
            let source = self.function_types[*source].clone();
            let target = self.function_types[*target].clone();
            for (source, target) in source
                .parameter_types
                .into_iter()
                .zip(target.parameter_types)
            {
                self.ensure_coercion_box_sources(target, source);
            }
            self.ensure_coercion_box_sources(source.return_type, target.return_type);
        } else if matches!(
            self.types[target].kind,
            concrete::TypeKind::Any | concrete::TypeKind::Interface(_)
        ) {
            self.ensure_box_source(source);
        }
    }
}
