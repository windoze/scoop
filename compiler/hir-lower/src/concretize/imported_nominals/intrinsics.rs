//! Imported intrinsic declarations use the same concrete nominal arenas.

use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn ensure_box_source(&mut self, ty: concrete::TypeId) {
        let (family, application) = match self.types[ty].kind {
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
        let source = self
            .source
            .imported_intrinsic_types
            .get(&family)
            .expect("boxed primitive HIR retains its complete dependency declaration");
        self.lower_imported_intrinsic_struct(source, family, ty, application);
    }

    fn lower_imported_intrinsic_struct(
        &mut self,
        source: &export::ImportedIntrinsicType,
        family: export::IntrinsicTypeKind,
        ty: concrete::TypeId,
        application: concrete::IntrinsicTypeRepresentation,
    ) {
        let declaration = &source.declaration;
        let identity = (declaration.owner(), Vec::new());
        if self.imported_structs.contains_key(&identity) {
            return;
        }
        let id = self.structs.alloc(concrete::StructDef {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: Vec::new(),
            gc_free: true,
            representation: concrete::StructRepresentation::Intrinsic {
                declaration: family,
                application,
            },
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            // The provider owns the callable bodies and boxing adapters.
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        self.imported_structs.insert(identity, id);
        self.struct_type.insert(id, ty);
        self.structs[id].direct_interfaces = source
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, &[]))
            .collect();
        self.structs[id].interfaces = self.lower_imported_value_interfaces(&source.interfaces, &[]);
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
