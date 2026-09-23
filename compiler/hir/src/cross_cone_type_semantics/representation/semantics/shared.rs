//! Representation joins use the same declarations as ordinary HIR lookup.

use scoop_identity::{
    CoreBuiltinNominal, ExactTypeKey, GeneratedNominalKey, OptionalSignatureType, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CanonicalNominalRepresentationSupportV1, CheckedNominalRepresentationSupportV1, source,
};
use crate::{
    CheckedExactTypeFactsV1, ExactTypeGcV1, NominalInterfaceRecordV1,
    NominalMaterializationClosure, NominalRepresentationShapeV1, NominalRepresentationSupportV1,
    PublicNominalKindV1,
    cross_cone_type_semantics::shared_foundation::{
        MetadataTypes, SharedTypeMetadataError as Error, declaration_access,
    },
};

impl CanonicalNominalRepresentationSupportV1 {
    pub(crate) fn validate_shared_metadata(
        &self,
        types: MetadataTypes<'_, '_>,
        materialization: &NominalMaterializationClosure,
        facts: CheckedExactTypeFactsV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedNominalRepresentationSupportV1<'_>, Error> {
        let path = WirePath::root();
        meter.charge_work(
            self.records().len() as u64 + materialization.sources().len() as u64,
            &path,
        )?;
        if !self
            .records()
            .iter()
            .map(|record| record.owner())
            .eq(materialization.sources().iter().copied())
        {
            return Err(Error::RepresentationInventory);
        }
        for record in self.records() {
            let owner = record.owner();
            let declaration = types.nominal(owner, meter)?;
            let key = types.nominal_key(owner, meter)?;
            let access = declaration_access(types.current, declaration, &key, meter)?;
            source::validate_header(
                record,
                &key,
                &access,
                types.current.provider,
                source_kind(declaration.kind()),
                meter,
                &path,
            )
            .map_err(|error| match error {
                source::Failure::Resource(error) => Error::Resource(error),
                source::Failure::Mismatch(_) => Error::Representation(owner),
            })?;
            if !record.public_source_shape_matches(declaration.source_shape(), meter, &path)? {
                return Err(Error::Representation(owner));
            }
            validate_relations(record, declaration, types, facts, meter)?;
        }
        Ok(CheckedNominalRepresentationSupportV1 { table: self })
    }
}

fn source_kind(kind: PublicNominalKindV1) -> SourceDeclarationKind {
    match kind {
        PublicNominalKindV1::Struct => SourceDeclarationKind::Struct,
        PublicNominalKindV1::Enum => SourceDeclarationKind::Enum,
        PublicNominalKindV1::Class => SourceDeclarationKind::Class,
        PublicNominalKindV1::Interface => SourceDeclarationKind::Interface,
        PublicNominalKindV1::Object => SourceDeclarationKind::Object,
    }
}

fn validate_relations(
    record: &NominalRepresentationSupportV1,
    declaration: &NominalInterfaceRecordV1,
    types: MetadataTypes<'_, '_>,
    facts: CheckedExactTypeFactsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    match record.shape() {
        NominalRepresentationShapeV1::Class { base, .. } => {
            validate_base(record.owner(), base, declaration, types, meter)
        }
        NominalRepresentationShapeV1::Enum { variants } => {
            for variant in variants {
                let mut expected = ExactTypeGcV1::GcFree;
                for field in variant.fields() {
                    let exact = types.exact(field.value_type(), 1, meter)?;
                    let gc = if let Some(fact) = facts.get(exact) {
                        fact.gc()
                    } else {
                        let key = types.key(exact, meter)?;
                        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
                            return Err(Error::MissingFact(exact));
                        };
                        let provider = types.nominal_key(*owner, meter)?.origin();
                        types.dependency_fact(provider, exact, meter)?.record().gc()
                    };
                    if gc != ExactTypeGcV1::GcFree {
                        expected = ExactTypeGcV1::ContainsManagedReferences;
                    }
                }
                if variant.gc() != expected {
                    return Err(Error::Representation(record.owner()));
                }
            }
            Ok(())
        }
        NominalRepresentationShapeV1::Object { backing_class, .. } => {
            meter.charge_work(1, &WirePath::root())?;
            let key = types
                .current
                .identities
                .canonical_key::<PersistentTypeId, GeneratedNominalKey>(*backing_class)?;
            if key.as_ref()
                != &(GeneratedNominalKey::ObjectBackingClass {
                    object: record.owner(),
                })
            {
                return Err(Error::Representation(record.owner()));
            }
            Ok(())
        }
        NominalRepresentationShapeV1::Struct { .. }
        | NominalRepresentationShapeV1::Interface
        | NominalRepresentationShapeV1::Intrinsic { .. } => Ok(()),
    }
}

fn validate_base(
    owner: PersistentTypeId,
    base: &OptionalSignatureType,
    declaration: &NominalInterfaceRecordV1,
    types: MetadataTypes<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut expected = None;
    for parent in declaration.exact_supertypes().values() {
        let SignatureTypeKey::Nominal(parent_owner) = parent else {
            return Err(Error::NonConcreteSignature);
        };
        let kind = if *parent_owner == CoreBuiltinNominal::Any.identity_record().id() {
            PublicNominalKindV1::Class
        } else {
            types.nominal(*parent_owner, meter)?.kind()
        };
        match kind {
            PublicNominalKindV1::Class => {
                if expected.replace(parent).is_some() {
                    return Err(Error::ClassBase(owner));
                }
            }
            PublicNominalKindV1::Interface => continue,
            _ => return Err(Error::ClassBase(owner)),
        }
    }
    let matches = match (base, expected) {
        (OptionalSignatureType::Absent, None) => true,
        (OptionalSignatureType::Present(actual), Some(expected)) => {
            NominalRepresentationSupportV1::signature_types_match_metered(
                actual,
                expected,
                1,
                meter,
                &WirePath::root(),
            )?
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Error::ClassBase(owner))
    }
}
