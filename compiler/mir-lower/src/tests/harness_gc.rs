use super::*;

impl Harness {
    /// Canonical source `UInt`.
    pub(super) fn uint(&mut self) -> hir::TypeId {
        self.uint
    }

    /// The pointer/GC raw carrier is source `ULong`, never `UInt`.
    pub(super) fn ulong(&self) -> hir::TypeId {
        self.ulong
    }

    /// Intern a generic struct application type.
    pub(super) fn struct_app(
        &mut self,
        struct_id: hir::StructId,
        args: Vec<hir::TypeId>,
    ) -> hir::TypeId {
        assert_eq!(self.structs[struct_id].type_params.len(), args.len());
        let application = self.struct_application(struct_id, args);
        self.struct_applications[application].canonical_type
    }

    /// core's GC facilities (M12): `PinnedPtr<T>` / `GcHandle<T>`
    /// and the six low-level runtime intrinsics, created on first use.
    pub(super) fn gc_core(&mut self) -> GcCore {
        if let Some(core) = self.gc_core {
            return core;
        }
        let ulong = self.ulong();
        let unit = self.unit;
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let pinned_ptr = self.declare_struct(
            "PinnedPtr",
            vec![type_param("T")],
            vec![t],
            &[("raw", ulong)],
            &[],
        );
        let gc_handle = self.declare_struct(
            "GcHandle",
            vec![type_param("T")],
            vec![t],
            &[("raw", ulong)],
            &[],
        );
        let mut dummy_locals = Arena::new();
        let mut intrinsic = |name: &str,
                             intrinsic: &str,
                             type_params: Vec<String>,
                             params: Vec<(&str, hir::TypeId)>,
                             return_ty: hir::TypeId| {
            let generic = !type_params.is_empty();
            let type_params = type_params.into_iter().map(type_param).collect();
            let id = self.functions.alloc(hir::Function {
                link_stem: callable_link_stem(name),
                name: name.to_string(),
                access: hir::DeclarationAccess::public(),
                override_access: Vec::new(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: params
                    .into_iter()
                    .map(|(name, ty)| hir::Param {
                        name: name.to_string(),
                        ty,
                        local: dummy_locals.alloc(hir::Local {
                            binding: hir::BindingId::from_raw(dummy_locals.len() as u32),
                            selector: test_local_selector(dummy_locals.len() as u32),
                            name: name.to_string(),
                            ty,
                            mutable: false,
                        }),
                    })
                    .collect(),
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::intrinsic_spec(intrinsic)
                        .expect("test intrinsic is registered")
                        .kind(),
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            if generic {
                self.register_generic(id, type_params);
            }
            self.top_level.push(id);
            id
        };
        let type_params = vec!["T".to_string()];
        let pin_raw = intrinsic(
            "_pin",
            "gc_pin_raw",
            type_params.clone(),
            vec![("v", t)],
            ulong,
        );
        let unpin_raw = intrinsic(
            "_unpin",
            "gc_unpin_raw",
            type_params.clone(),
            vec![("raw", ulong)],
            t,
        );
        let get_handle_raw = intrinsic(
            "_getGcHandle",
            "gc_get_handle_raw",
            type_params.clone(),
            vec![("v", t)],
            ulong,
        );
        let release_handle_raw = intrinsic(
            "_releaseGcHandle",
            "gc_release_handle_raw",
            type_params,
            vec![("raw", ulong)],
            t,
        );
        let gc_collect = intrinsic("gcCollect", "rt_gc_collect", vec![], vec![], unit);
        let gc_stats = intrinsic("gcStats", "rt_gc_stats", vec![], vec![], ulong);
        let core = GcCore {
            pinned_ptr,
            gc_handle,
            pin_raw,
            unpin_raw,
            get_handle_raw,
            release_handle_raw,
            gc_collect,
            gc_stats,
        };
        self.gc_core = Some(core);
        core
    }

    pub(super) fn tuple(&mut self, elements: &[hir::TypeId]) -> hir::TypeId {
        self.types.alloc(hir::Type::Tuple(elements.to_vec()))
    }

    pub(super) fn intrinsic_array_class(&mut self, kind: hir::IntrinsicTypeKind) -> hir::ClassId {
        let existing = match kind {
            hir::IntrinsicTypeKind::Array => self.intrinsic_array,
            hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array,
            _ => unreachable!("array helper accepts only intrinsic array families"),
        };
        if let Some(class) = existing {
            return class;
        }
        let parameter = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let class = self.declare_intrinsic_class(
            kind.source_name(),
            kind,
            vec![type_param("T")],
            vec![parameter],
            CanonicalTypePlan::Allocate,
        );
        match kind {
            hir::IntrinsicTypeKind::Array => self.intrinsic_array = Some(class),
            hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array = Some(class),
            _ => unreachable!("array helper accepts only intrinsic array families"),
        }
        class
    }

    pub(super) fn array(&mut self, element: hir::TypeId) -> hir::TypeId {
        let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
        let application = self.class_application(class, vec![element]);
        self.class_applications[application].canonical_type
    }

    pub(super) fn mutable_array(&mut self, element: hir::TypeId) -> hir::TypeId {
        let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
        let application = self.class_application(class, vec![element]);
        self.class_applications[application].canonical_type
    }
}
