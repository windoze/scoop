//! Structural immutability predicate used as the immortal constantization gate.
//!
//! P5-T02 wires this predicate into emission; P5-T01 lands the reusable analysis
//! with unit coverage first.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::ast;
use crate::effect_lowered::source as hir;
use crate::ty::{
    MonoTypeId, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
    is_builtin_scalar_nominal_value_type,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImmutabilityMemoState {
    Visiting,
    Done(bool),
}

/// Computes `is_immutable(T)` from type and field metadata.
pub(in crate::llvm::codegen) struct TypeImmutability<'a> {
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    class_inits: &'a hir::ClassInitIndex,
    nominal_kinds: &'a hir::NominalKindIndex,
    interior_mutable_nominals: &'a hir::InteriorMutableIndex,
    memo: HashMap<TypeId, ImmutabilityMemoState>,
    class_memo: HashMap<hir::ClassInstanceKey, ImmutabilityMemoState>,
}

impl<'a> TypeImmutability<'a> {
    pub(in crate::llvm::codegen) fn new(
        types: &'a TypeStore,
        struct_layouts: &'a hir::StructLayoutIndex,
        class_inits: &'a hir::ClassInitIndex,
        nominal_kinds: &'a hir::NominalKindIndex,
        interior_mutable_nominals: &'a hir::InteriorMutableIndex,
    ) -> Self {
        Self {
            types,
            struct_layouts,
            class_inits,
            nominal_kinds,
            interior_mutable_nominals,
            memo: HashMap::new(),
            class_memo: HashMap::new(),
        }
    }

    pub(in crate::llvm::codegen) fn is_immutable(&mut self, ty: TypeId) -> bool {
        self.is_immutable_inner(ty)
    }

    fn is_immutable_inner(&mut self, ty: TypeId) -> bool {
        match self.memo.get(&ty).copied() {
            Some(ImmutabilityMemoState::Done(result)) => return result,
            Some(ImmutabilityMemoState::Visiting) => return true,
            None => {}
        }

        self.memo.insert(ty, ImmutabilityMemoState::Visiting);
        let result = self.compute_is_immutable(ty);
        self.memo.insert(ty, ImmutabilityMemoState::Done(result));
        result
    }

    fn compute_is_immutable(&mut self, ty: TypeId) -> bool {
        if self.nominal_has_interior_mutable(ty) {
            return false;
        }

        if is_builtin_scalar_nominal_value_type(self.types, ty) {
            return true;
        }

        match self.types.kind(ty).clone() {
            TypeKind::Value(value) => self.value_type_is_immutable(value),
            TypeKind::Ref(reference) => self.ref_type_is_immutable(ty, reference),
            TypeKind::StarProjection(_) | TypeKind::Param(_) => false,
        }
    }

    fn value_type_is_immutable(&mut self, value: ValueTypeKind) -> bool {
        match value {
            ValueTypeKind::Unit
            | ValueTypeKind::Nothing
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_) => true,
            ValueTypeKind::Option(inner) => self.is_immutable_inner(inner),
            ValueTypeKind::Tuple(elements) => elements
                .into_iter()
                .all(|element| self.is_immutable_inner(element)),
            ValueTypeKind::Nominal(nominal) => self.value_nominal_is_immutable(&nominal),
        }
    }

    fn ref_type_is_immutable(&mut self, ty: TypeId, reference: RefTypeKind) -> bool {
        match reference {
            RefTypeKind::String => true,
            RefTypeKind::Nominal(nominal) => self.ref_nominal_is_immutable(ty, &nominal),
            RefTypeKind::Any | RefTypeKind::Function(_) | RefTypeKind::Union(_) => false,
        }
    }

    fn value_nominal_is_immutable(&mut self, nominal: &NominalType) -> bool {
        if self.nominal_kinds.get(&nominal.fqn) != Some(&ast::TypeKind::Struct) {
            return false;
        }

        let key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, self.types);
        let Some(layout) = self.struct_layouts.get(&key) else {
            return false;
        };
        let Some(field_tys) = layout
            .fields
            .iter()
            .map(|field| field.ty.map(MonoTypeId::inner))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };

        field_tys
            .into_iter()
            .all(|field_ty| self.is_immutable_inner(field_ty))
    }

    fn ref_nominal_is_immutable(&mut self, ty: TypeId, nominal: &NominalType) -> bool {
        if self.nominal_kinds.get(&nominal.fqn) != Some(&ast::TypeKind::Class) {
            return false;
        }

        let Ok(mono_ty) = self.types.as_mono(ty) else {
            return false;
        };
        let Some(class_key) = hir::ClassInstanceKey::from_mono_nominal(self.types, mono_ty) else {
            return false;
        };
        self.class_is_immutable(&class_key)
    }

    fn class_is_immutable(&mut self, class_key: &hir::ClassInstanceKey) -> bool {
        match self.class_memo.get(class_key).copied() {
            Some(ImmutabilityMemoState::Done(result)) => return result,
            Some(ImmutabilityMemoState::Visiting) => return true,
            None => {}
        }

        self.class_memo
            .insert(class_key.clone(), ImmutabilityMemoState::Visiting);
        let result = self.compute_class_is_immutable(class_key);
        self.class_memo
            .insert(class_key.clone(), ImmutabilityMemoState::Done(result));
        result
    }

    fn compute_class_is_immutable(&mut self, class_key: &hir::ClassInstanceKey) -> bool {
        let Some(class) = self.class_inits.get(class_key).cloned() else {
            return false;
        };
        if self.interior_mutable_nominals.contains(&class.fqn) {
            return false;
        }

        if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            let Some(super_key) = self.registered_class_instance_key(super_fqn) else {
                return false;
            };
            if !self.class_is_immutable(&super_key) {
                return false;
            }
        }

        class
            .fields
            .into_iter()
            .all(|field| !field.mutable && self.is_immutable_inner(field.ty.inner()))
    }

    fn registered_class_instance_key(&self, class_fqn: &str) -> Option<hir::ClassInstanceKey> {
        self.class_inits
            .keys()
            .find(|key| key.as_str() == class_fqn)
            .cloned()
    }

    fn nominal_has_interior_mutable(&self, ty: TypeId) -> bool {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                self.interior_mutable_nominals.contains(&nominal.fqn)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::span::Span;

    #[derive(Default)]
    struct FixtureTypes {
        types: TypeStore,
        struct_layouts: hir::StructLayoutIndex,
        class_inits: hir::ClassInitIndex,
        nominal_kinds: hir::NominalKindIndex,
        interior_mutable_nominals: hir::InteriorMutableIndex,
    }

    impl FixtureTypes {
        fn analyzer(&self) -> TypeImmutability<'_> {
            TypeImmutability::new(
                &self.types,
                &self.struct_layouts,
                &self.class_inits,
                &self.nominal_kinds,
                &self.interior_mutable_nominals,
            )
        }

        fn add_value_struct(&mut self, fqn: &str, fields: Vec<(&str, TypeId)>) -> TypeId {
            self.nominal_kinds
                .insert(fqn.to_string(), ast::TypeKind::Struct);
            let nominal = NominalType {
                fqn: fqn.to_string(),
                args: vec![],
                eff: None,
            };
            let ty = self
                .types
                .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal)));
            let layout_fields = fields
                .into_iter()
                .map(|(name, field_ty)| hir::StructFieldLayout {
                    span: Span::synthetic_prelude(),
                    name: name.to_string(),
                    fqn: format!("{fqn}.{name}"),
                    ty: Some(self.types.as_mono(field_ty).expect("field type is mono")),
                    ty_fqn: None,
                })
                .collect();
            self.struct_layouts.insert(
                fqn.to_string(),
                hir::StructLayout {
                    fqn: fqn.to_string(),
                    fields: layout_fields,
                    c_layout: None,
                },
            );
            ty
        }

        fn add_ref_class(&mut self, fqn: &str, fields: Vec<(&str, bool, TypeId)>) -> TypeId {
            self.nominal_kinds
                .insert(fqn.to_string(), ast::TypeKind::Class);
            let nominal = NominalType {
                fqn: fqn.to_string(),
                args: vec![],
                eff: None,
            };
            let ty = self
                .types
                .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal)));
            let mono_ty = self.types.as_mono(ty).expect("class type is mono");
            let class_key = hir::ClassInstanceKey::from_mono_nominal(&self.types, mono_ty)
                .expect("class key from nominal");
            let mut field_indices = HashMap::new();
            let class_fields = fields
                .into_iter()
                .enumerate()
                .map(|(idx, (name, mutable, field_ty))| {
                    let field_fqn = format!("{fqn}.{name}");
                    field_indices.insert(field_fqn.clone(), idx as u32);
                    hir::ClassField {
                        fqn: field_fqn,
                        name: name.to_string(),
                        mutable,
                        ty: self.types.as_mono(field_ty).expect("field type is mono"),
                    }
                })
                .collect();
            self.class_inits.insert(
                class_key,
                hir::MonoClassInit {
                    fqn: fqn.to_string(),
                    source_path: PathBuf::new(),
                    super_class_fqn: None,
                    super_ctor_args_span: None,
                    super_ctor_call: None,
                    super_ctor_args: vec![],
                    this_id: hir::SymbolId::from_raw(0),
                    fields: class_fields,
                    field_indices,
                    steps: vec![],
                    ctors: vec![],
                },
            );
            ty
        }

        fn add_interior_mutable_struct(
            &mut self,
            fqn: &str,
            fields: Vec<(&str, TypeId)>,
        ) -> TypeId {
            let ty = self.add_value_struct(fqn, fields);
            self.interior_mutable_nominals.insert(fqn.to_string());
            ty
        }
    }

    #[test]
    fn string_struct_tuple_and_all_val_class_are_immutable() {
        let mut fixture = FixtureTypes::default();
        let builtins = fixture.types.intern_builtins();
        let tuple = fixture
            .types
            .intern(TypeKind::Value(ValueTypeKind::Tuple(vec![
                builtins.int,
                builtins.string,
            ])));
        let point = fixture.add_value_struct(
            "fixture.Point",
            vec![("x", builtins.int), ("name", builtins.string)],
        );
        let record = fixture.add_ref_class(
            "fixture.Record",
            vec![("point", false, point), ("pair", false, tuple)],
        );

        let mut analyzer = fixture.analyzer();
        assert!(analyzer.is_immutable(builtins.string));
        assert!(analyzer.is_immutable(tuple));
        assert!(analyzer.is_immutable(point));
        assert!(analyzer.is_immutable(record));
    }

    #[test]
    fn var_fields_nested_mutable_refs_and_interior_mutable_types_are_rejected() {
        let mut fixture = FixtureTypes::default();
        let builtins = fixture.types.intern_builtins();
        let ref_cell =
            fixture.add_ref_class("fixture.RefCell", vec![("value", true, builtins.int)]);
        let atomic_int =
            fixture.add_ref_class("fixture.AtomicInt", vec![("raw", true, builtins.int)]);
        let atomic_storage = fixture
            .add_interior_mutable_struct("scoop.unsafe.__AtomicInt", vec![("raw", builtins.int)]);
        let val_ref_cell_holder =
            fixture.add_ref_class("fixture.HoldsRefCell", vec![("cell", false, ref_cell)]);
        let var_ref_cell_holder =
            fixture.add_ref_class("fixture.VarHoldsRefCell", vec![("cell", true, ref_cell)]);
        let marked_class =
            fixture.add_ref_class("fixture.MarkedCell", vec![("raw", false, builtins.int)]);
        fixture
            .interior_mutable_nominals
            .insert("fixture.MarkedCell".to_string());

        let mut analyzer = fixture.analyzer();
        assert!(!analyzer.is_immutable(ref_cell));
        assert!(!analyzer.is_immutable(atomic_int));
        assert!(!analyzer.is_immutable(atomic_storage));
        assert!(!analyzer.is_immutable(val_ref_cell_holder));
        assert!(!analyzer.is_immutable(var_ref_cell_holder));
        assert!(!analyzer.is_immutable(marked_class));
    }

    #[test]
    fn recursive_all_val_classes_terminate_as_immutable() {
        let mut fixture = FixtureTypes::default();
        fixture.types.intern_builtins();
        let node = fixture.add_ref_class("fixture.Node", vec![]);
        let mono_node = fixture.types.as_mono(node).expect("node type is mono");
        let class_key = hir::ClassInstanceKey::from_mono_nominal(&fixture.types, mono_node)
            .expect("class key from nominal");
        let class = fixture
            .class_inits
            .get_mut(&class_key)
            .expect("node class exists");
        class
            .field_indices
            .insert("fixture.Node.next".to_string(), 0);
        class.fields.push(hir::ClassField {
            fqn: "fixture.Node.next".to_string(),
            name: "next".to_string(),
            mutable: false,
            ty: mono_node,
        });

        let mut analyzer = fixture.analyzer();
        assert!(analyzer.is_immutable(node));
    }
}
