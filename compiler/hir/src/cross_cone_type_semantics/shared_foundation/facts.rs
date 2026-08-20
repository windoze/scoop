use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, SignatureTypeKey};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalExactTypeFactsV1, CheckedExactTypeFactV1, EnumSourceShapeV1, ExactEnumVariantFactsV1,
    ExactTypeFactShapeV1 as Shape, ExactTypeFactsDependencyLookupV1,
    ExactTypeFactsSemanticAuthority, ExactTypeFactsV1, ExactTypeGcV1, IntrinsicTypeKind,
    NominalCLayoutPolicyV1, NominalSourceShapeV1, SourceNominalId,
};

pub(super) fn validate<'a>(
    candidate: &'a CanonicalExactTypeFactsV1,
    types: MetadataTypes<'a, '_>,
    materialization: &NominalMaterializationClosure,
) -> Result<CheckedExactTypeFactsV1<'a>, Error> {
    let mut replay = Replay {
        candidate,
        types,
        shapes: BTreeMap::new(),
        foreign: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    super::requirements::project(&mut replay, materialization)?;

    if !candidate
        .records()
        .iter()
        .map(|record| record.exact())
        .eq(replay.shapes.keys().copied())
    {
        return Err(Error::FactInventory);
    }
    candidate
        .validate_semantics_with_dependencies(&replay, &replay)
        .map_err(|error| Error::Facts(Box::new(error)))
}

pub(super) struct Replay<'a, 'd> {
    candidate: &'a CanonicalExactTypeFactsV1,
    pub(super) types: MetadataTypes<'a, 'd>,
    shapes: BTreeMap<PersistentExactTypeId, Shape>,
    foreign: BTreeMap<PersistentExactTypeId, CheckedExactTypeFactV1<'a>>,
    active: BTreeSet<PersistentExactTypeId>,
}

impl Replay<'_, '_> {
    pub(super) fn signature(
        &mut self,
        signature: &SignatureTypeKey,
    ) -> Result<PersistentExactTypeId, Error> {
        let exact = self.types.exact(signature)?;
        self.visit(exact)?;
        Ok(exact)
    }

    pub(super) fn visit(&mut self, exact: PersistentExactTypeId) -> Result<(), Error> {
        let path = WirePath::root();

        if self.shapes.contains_key(&exact) || self.foreign.contains_key(&exact) {
            return Ok(());
        }
        let key = self.types.key(exact)?;
        if let ExactTypeKey::Nominal(owner) = key.as_ref() {
            let provider = self.types.nominal_key(*owner)?.origin();
            if provider != self.types.current.provider {
                if self.candidate.get(exact).is_some() {
                    return Err(Error::ForeignFact(exact));
                }
                let fact = self.types.dependency_fact(provider, exact)?;

                self.foreign.insert(exact, fact);
                return Ok(());
            }
        }

        if !self.active.insert(exact) {
            return Err(Error::ByValueCycle(exact));
        }
        let shape = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => {
                self.nominal_shape(SourceNominalId::Concrete(*owner), &[])?
            }
            ExactTypeKey::Tuple(elements) => {
                let mut children = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut children,
                    elements.as_slice().len(),
                    &path,
                )?;
                for element in elements.as_slice() {
                    self.visit(*element)?;
                    children.push(*element);
                }
                Shape::Tuple { elements: children }
            }
            ExactTypeKey::RawPointer(_) | ExactTypeKey::NativeFunctionPointer { .. } => {
                Shape::Pointer
            }
            ExactTypeKey::Function { .. } => Shape::Reference,
            ExactTypeKey::NominalApplication { origin, arguments } => self.nominal_shape(
                SourceNominalId::GenericTemplate(*origin),
                &[arguments.as_slice().to_vec()],
            )?,
        };
        self.active.remove(&exact);

        self.shapes.insert(exact, shape);
        Ok(())
    }

    fn nominal_shape(
        &mut self,
        owner: SourceNominalId,
        bindings: &[Vec<PersistentExactTypeId>],
    ) -> Result<Shape, Error> {
        if owner == SourceNominalId::Concrete(CoreBuiltinNominal::Unit.identity_record().id()) {
            return Ok(Shape::Unit);
        }
        let nominal = self.types.nominal_declaration(owner)?;
        match nominal.source_shape() {
            NominalSourceShapeV1::Struct(source) => {
                let fields = self.fields(
                    source.fields().iter().map(|field| field.value_type()),
                    bindings,
                )?;
                Ok(match source.c_layout_policy() {
                    NominalCLayoutPolicyV1::Ordinary => Shape::OrdinaryStruct { fields },
                    NominalCLayoutPolicyV1::CLayout { .. } => Shape::CLayoutStruct { fields },
                })
            }
            NominalSourceShapeV1::Enum(source) => self.enumeration(source, bindings),
            NominalSourceShapeV1::Class(_)
            | NominalSourceShapeV1::Object(_)
            | NominalSourceShapeV1::Interface => Ok(Shape::Reference),
            NominalSourceShapeV1::Intrinsic(representation) => match representation.family() {
                IntrinsicTypeKind::MaybeUninit => {
                    let fields = self.fields(
                        [&SignatureTypeKey::Binder { depth: 0, index: 0 }].into_iter(),
                        bindings,
                    )?;
                    Ok(Shape::MaybeUninit { value: fields[0] })
                }
                IntrinsicTypeKind::Unit => Ok(Shape::Unit),
                IntrinsicTypeKind::Integer(_)
                | IntrinsicTypeKind::Float(_)
                | IntrinsicTypeKind::Char
                | IntrinsicTypeKind::Boolean => Ok(Shape::Scalar),
                IntrinsicTypeKind::String
                | IntrinsicTypeKind::Atomic(_)
                | IntrinsicTypeKind::Any
                | IntrinsicTypeKind::Nothing
                | IntrinsicTypeKind::Array
                | IntrinsicTypeKind::MutableArray => Ok(Shape::Reference),
                IntrinsicTypeKind::Ptr | IntrinsicTypeKind::FunPtr => Ok(Shape::Pointer),
            },
        }
    }

    fn enumeration(
        &mut self,
        source: &EnumSourceShapeV1,
        bindings: &[Vec<PersistentExactTypeId>],
    ) -> Result<Shape, Error> {
        let mut variants = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut variants,
            source.variants().len(),
            &WirePath::root(),
        )?;
        for variant in source.variants() {
            let fields = self.fields(
                variant.fields().iter().map(|field| field.value_type()),
                bindings,
            )?;
            // Every field is recursively replayed by the shared facts validator.
            // This derived annotation is checked against those same child facts.
            let mut gc = ExactTypeGcV1::GcFree;
            for field in &fields {
                if self.fact(*field)?.gc() != ExactTypeGcV1::GcFree {
                    gc = ExactTypeGcV1::ContainsManagedReferences;
                }
            }
            variants.push(ExactEnumVariantFactsV1 {
                variant: variant.variant(),
                fields,
                gc,
            });
        }
        Ok(Shape::Enum { variants })
    }

    fn fields<'s>(
        &mut self,
        fields: impl ExactSizeIterator<Item = &'s SignatureTypeKey>,
        bindings: &[Vec<PersistentExactTypeId>],
    ) -> Result<Vec<PersistentExactTypeId>, Error> {
        let mut exacts = Vec::new();
        scoop_wire::allocation::try_reserve(&mut exacts, fields.len(), &WirePath::root())?;
        for field in fields {
            let exact = self.types.exact_with_bindings(field, bindings)?;
            self.visit(exact)?;
            exacts.push(exact);
        }
        Ok(exacts)
    }

    fn fact(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeFactsV1, Error> {
        self.candidate
            .get(exact)
            .or_else(|| self.foreign.get(&exact).map(|fact| fact.record()))
            .ok_or(Error::MissingFact(exact))
    }
}

impl ExactTypeFactsSemanticAuthority<Error> for Replay<'_, '_> {
    fn fact_shape(&self, exact: PersistentExactTypeId) -> Result<&Shape, Error> {
        self.shapes.get(&exact).ok_or(Error::MissingFact(exact))
    }
}

impl ExactTypeFactsDependencyLookupV1 for Replay<'_, '_> {
    fn get_dependency_fact(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<CheckedExactTypeFactV1<'_>> {
        self.foreign.get(&exact).copied()
    }
}
