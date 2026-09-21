use super::source_dispatch::with_hir_source;
use super::*;
use hir::{DefaultSourceBodyProductionError as Error, DefaultSourceBodyProductionV1 as Body};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{ResourceKind, WireErrorKind};

mod budgets;
mod protocols;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/bodies.scoop"
));
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn function(export: &hir::ExportHir, name: &str) -> hir::ExportParameterOwner {
    hir::ExportParameterOwner::Function(
        export
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0,
    )
}
fn calling_source(calling: hir::ExportParameterCalling) -> Option<hir::ExportDefaultSourceId> {
    match calling {
        hir::ExportParameterCalling::Default { source, .. }
        | hir::ExportParameterCalling::Vararg {
            omission: hir::ExportVarargOmission::Default(source),
            ..
        } => Some(source),
        _ => None,
    }
}

#[test]
fn source_body_projection_covers_restricted_and_inherited_defaults() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let public = hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output).unwrap();
        let mut summary = Vec::new();
        for interface in &export.source_parameter_interfaces {
            for (position, parameter) in interface.parameters.iter().enumerate() {
                let Some(source) = calling_source(parameter.calling) else {
                    continue;
                };
                let projected = Body::from_dependency_hir(
                    output,
                    interface.owner,
                    position as u32,
                    &mut meter(),
                )
                .unwrap();
                let original =
                    &export.export_default_exprs[export.export_default_sources[source].expression];
                assert!(std::ptr::eq(
                    projected.source_references(),
                    &original.references
                ));
                assert_eq!(projected.definition_path(), &original.definition_path);
                assert_eq!(projected.locals().records().len(), original.locals.len());
                let again = Body::from_dependency_hir(
                    output,
                    interface.owner,
                    position as u32,
                    &mut meter(),
                )
                .unwrap();
                assert_eq!(again.body(), projected.body());
                assert_eq!(again.locals(), projected.locals());
                if let Some(template) = public.get(hir::ExportDefaultTemplateKeyV1::new(
                    projected.owner(),
                    position as u32,
                )) {
                    assert_eq!(projected.body(), template.body());
                    assert_eq!(projected.locals(), template.locals());
                    assert_eq!(projected.definition_root(), template.definition_root());
                    assert_eq!(projected.type_parameters(), template.type_parameters());
                    assert_eq!(projected.definition_origin(), template.definition_origin());
                }
                let name = match interface.owner {
                    hir::ExportParameterOwner::Function(id) => export.functions[id].name.clone(),
                    hir::ExportParameterOwner::StructConstructor(_) => "Value.constructor".into(),
                    hir::ExportParameterOwner::ClassConstructor(_) => "Base.constructor".into(),
                    hir::ExportParameterOwner::VariantConstructor(_) => "Choice.Item".into(),
                };
                summary.push(format!(
                    "{name}[{position}]: locals={}, statements={}, binders={}, inherited={}",
                    projected.locals().records().len(),
                    projected.body().statements().len(),
                    projected.provider_binders().len(),
                    projected.owner() != projected.definition_root().declaration()
                ));
            }
        }
        summary.sort();
        assert_eq!(
            summary.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/bodies.snap"
            ))
        );
        let child =
            Body::from_dependency_hir(output, function(export, "Child.choose"), 1, &mut meter())
                .unwrap();
        let parent =
            Body::from_dependency_hir(output, function(export, "Base.choose"), 1, &mut meter())
                .unwrap();
        assert_ne!(child.owner(), parent.owner());
        assert_eq!(child.body(), parent.body());
        assert_eq!(child.definition_root(), parent.definition_root());
        let generic =
            Body::from_dependency_hir(output, function(export, "Base.generic"), 1, &mut meter())
                .unwrap();
        assert!(matches!(
            generic.owner(),
            CallableTemplateOrigin::GenericFunction(_)
        ));
        assert_eq!(generic.provider_binders().len(), 1);
        assert_eq!(
            generic.type_parameters().arguments(),
            &[scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }]
        );
        let callback =
            Body::from_dependency_hir(output, function(export, "Base.callback"), 1, &mut meter())
                .unwrap();
        let hir::DefaultExpressionKindV1::Lambda(lambda) = callback.body().value().kind() else {
            panic!("lambda descriptor required")
        };
        assert_eq!(lambda.captures().len(), 1);
    });
}

#[test]
fn source_body_rejects_missing_duplicate_and_required_parameter_sources() {
    with_hir_source(SOURCE, |output, _| {
        let mut export = output.output().export.module().clone();
        let owner = function(&export, "Base.choose");
        assert!(matches!(
            Body::from_export_hir(&export, owner, 0, &mut meter()),
            Err(Error::NoDefault { position: 0, .. })
        ));
        assert!(matches!(
            Body::from_export_hir(&export, owner, 2, &mut meter()),
            Err(Error::MissingParameter { position: 2, .. })
        ));
        let index = export
            .source_parameter_interfaces
            .iter()
            .position(|source| source.owner == owner)
            .unwrap();
        export
            .source_parameter_interfaces
            .push(export.source_parameter_interfaces[index].clone());
        assert!(matches!(
            Body::from_export_hir(&export, owner, 1, &mut meter()),
            Err(Error::DuplicateInterface(_))
        ));
        export
            .source_parameter_interfaces
            .retain(|source| source.owner != owner);
        assert!(matches!(
            Body::from_export_hir(&export, owner, 1, &mut meter()),
            Err(Error::MissingInterface(_))
        ));
    });
}
