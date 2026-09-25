use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{CanonicalNominalSourceCallablesV1 as Table, NominalSupportPropertyPayloadV1};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

mod contracts;
mod render;
mod shapes;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/callables.scoop"
));
const PROPERTIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/properties.scoop"
));
const PROTECTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));

fn required(output: &hir::DependencyHirOutput) -> BTreeSet<CallableTemplateOrigin> {
    let export = &output.output().export;
    let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(export).unwrap();
    let nominals = hir::CanonicalNominalSourceContractsV1::from_export_hir(export, &roots).unwrap();
    let mut required = BTreeSet::new();
    let mut properties = Vec::new();
    for nominal in nominals.records() {
        for member in nominal.members().values() {
            match member {
                hir::NestedSourceMemberRefV1::Function(id) => {
                    required.insert(CallableTemplateOrigin::Function(*id));
                }
                hir::NestedSourceMemberRefV1::GenericFunction(id) => {
                    required.insert(CallableTemplateOrigin::GenericFunction(*id));
                }
                hir::NestedSourceMemberRefV1::Property(id) => properties.push(*id),
            }
        }
        if let hir::NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
            required.extend(
                shape
                    .variants()
                    .iter()
                    .map(|variant| CallableTemplateOrigin::VariantConstructor(variant.variant())),
            );
        }
    }
    let properties = hir::CanonicalNominalSourcePropertiesV1::from_export_hir(
        export,
        &hir::CanonicalPersistentIdsV1::try_new(properties).unwrap(),
    )
    .unwrap();
    for property in properties.records() {
        if let NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload() {
            required.insert(CallableTemplateOrigin::Accessor(interface.getter()));
            if let hir::ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
                interface.mutability()
            {
                required.insert(CallableTemplateOrigin::Accessor(*setter));
            }
        }
    }
    required
}
fn table(output: &hir::DependencyHirOutput) -> Table {
    Table::from_export_hir(&output.output().export, &required(output)).unwrap()
}

#[test]
fn nominal_callable_sources_preserve_generic_members_all_accessors_and_variants() {
    for input in [SOURCE, PROPERTIES, PROTECTED] {
        with_source(input, |output, _| {
            let table = table(output);
            assert_eq!(table.records().len(), required(output).len());
            contracts::verify(output, &table);
        });
    }
}

#[test]
fn complete_and_protected_callable_sources_share_the_same_contracts() {
    with_source(PROTECTED, |output, _| {
        let complete = table(output);
        let protected =
            hir::CanonicalInheritanceSourceProtectedCallablesV1::from_dependency_hir(output)
                .unwrap();
        assert!(complete.records().len() > protected.records().len());
        for record in protected.records() {
            let source = complete.get(record.declaration()).unwrap();
            assert_eq!(
                hir::ProtectedCallableInterfaceV1::try_from(source.clone()).unwrap(),
                *record
            );
            assert_eq!(encode(source).unwrap(), encode(record).unwrap());
        }
        for record in complete.records() {
            if record.declaration_access().declared_visibility()
                != hir::DeclaredVisibilityV1::Protected
            {
                assert!(hir::ProtectedCallableInterfaceV1::try_from(record.clone()).is_err());
            }
        }
    });
}

#[test]
fn nominal_callable_projection_rejects_top_level_constructors_and_const_accessors() {
    with_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let (id, _) = export
            .functions
            .iter()
            .find(|(_, f)| f.name == "topLevel")
            .unwrap();
        let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
            &export.function_identities[id]
        else {
            panic!("source function");
        };
        let top = CallableTemplateOrigin::Function(identity.id());
        let (_, property) = export
            .properties
            .iter()
            .find(|(_, p)| p.name == "fixed")
            .unwrap();
        let constant = CallableTemplateOrigin::Accessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let (constructor, _) = export.class_constructors.iter().next().unwrap();
        let constructor = CallableTemplateOrigin::Constructor(
            export.constructor_identities[constructor]
                .source_record()
                .unwrap()
                .id(),
        );
        for declaration in [top, constant, constructor] {
            assert!(matches!(
                Table::from_export_hir(&output.output().export, &BTreeSet::from([declaration])),
                Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
            ));
        }
    });
}
