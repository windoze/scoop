//! Representation joins use the same declarations as ordinary HIR lookup.

use scoop_identity::{
    ExactTypeKey, GeneratedNominalKey, OptionalSignatureType, PersistentTypeId,
    SourceDeclarationKind,
};
use scoop_wire::WirePath;

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
    ) -> Result<CheckedNominalRepresentationSupportV1<'_>, Error> {
        let path = WirePath::root();

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
            let declaration = types.nominal(owner)?;
            let key = types.nominal_key(owner)?;
            let access = declaration_access(types.current, declaration, &key)?;
            source::validate_header(
                record,
                &key,
                &access,
                types.current.provider,
                source_kind(declaration.kind()),
            )
            .map_err(|error| match error {
                source::Failure::Resource(error) => Error::Resource(error),
                source::Failure::Mismatch(_) => Error::Representation(owner),
            })?;
            if !record.public_source_shape_matches(declaration.source_shape(), &path)? {
                return Err(Error::Representation(owner));
            }
            validate_relations(record, declaration, types, facts)?;
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
) -> Result<(), Error> {
    match record.shape() {
        NominalRepresentationShapeV1::Class { base, .. } => {
            validate_base(record.owner(), base, declaration, types)
        }
        NominalRepresentationShapeV1::Enum { variants } => {
            for variant in variants {
                let mut expected = ExactTypeGcV1::GcFree;
                for field in variant.fields() {
                    let exact = types.exact(field.value_type())?;
                    let gc = if let Some(fact) = facts.get(exact) {
                        fact.gc()
                    } else {
                        let key = types.key(exact)?;
                        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
                            return Err(Error::MissingFact(exact));
                        };
                        let provider = types.nominal_key(*owner)?.origin();
                        types.dependency_fact(provider, exact)?.record().gc()
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
) -> Result<(), Error> {
    let mut expected = None;
    for parent in declaration.exact_supertypes().values() {
        let kind = types
            .applied_nominal(types.exact(parent)?)?
            .declaration
            .kind();
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
            crate::compare_default_signature_reference_targets(actual, expected, &WirePath::root())
                .map(|ordering| ordering.is_eq())?
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Error::ClassBase(owner))
    }
}
