use scoop_identity::{ConeCoordinate, ConeIdentity};

use super::m23_ordinary_core_only::support::{TrustedCoreFixture, parsed_ordinary, trusted_core};
use super::m23_ordinary_dependencies::support::{
    certificate, empty_alias_expansions, empty_interface, exact_import,
    project_dependency_without_default_roles,
};
use super::{
    Decl, Expr, TypeRef, bool_lit, file, fun_expr, ident, int_lit, make_core_public, return_value,
    sp, str_lit, ty_named, var,
};
use crate::{OrdinarySources, lower_ordinary};

struct DependencyConstantFixture {
    core: TrustedCoreFixture,
    coordinate: ConeCoordinate,
    foundation: scoop_hir::ImportedHirFoundation,
    interface: scoop_hir::CrossConeHirInterfaceSectionV1,
    aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    core_interface: scoop_hir::CrossConeHirInterfaceSectionV1,
}

impl DependencyConstantFixture {
    fn new() -> Self {
        let mut core = trusted_core();
        let coordinate = ConeCoordinate::new("test", "constant-provider", "1.0.0").unwrap();
        let mut provider = file(vec![
            const_property("answer", ty_named("Int"), int_lit(42)),
            const_property("enabled", ty_named("Boolean"), bool_lit(true)),
            const_property("label", ty_named("String"), str_lit("forty-two")),
        ]);
        make_core_public(&mut provider);
        provider.package = scoop_ast::PackageSyntax::QualifiedPackage {
            package_keyword_span: sp(),
            path: qualified(&["dependency", "api"]),
            span: sp(),
        };
        let (foundation, interface) = project_dependency_without_default_roles(
            &core,
            &coordinate,
            provider,
            &["Int", "Boolean", "String"],
        );
        let foundation = core.import_dependency_foundation(&coordinate, &foundation, 56);
        Self {
            core,
            coordinate,
            foundation,
            interface,
            aliases: empty_alias_expansions(),
            core_interface: empty_interface(),
        }
    }

    fn inspect<R>(
        &self,
        consumer: scoop_ast::SourceFile,
        inspect: impl FnOnce(scoop_hir::OrdinaryHirOutput<'_>) -> R,
    ) -> R {
        let ordinary = parsed_ordinary(consumer);
        let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
            ordinary.cone(),
            vec![
                scoop_hir::DirectImportedProviderInput::from_validated(
                    certificate(&ConeCoordinate::reserved_core(), 41),
                    &self.core.foundation,
                    &self.core_interface,
                    &self.aliases,
                ),
                scoop_hir::DirectImportedProviderInput::from_validated(
                    certificate(&self.coordinate, 56),
                    &self.foundation,
                    &self.interface,
                    &self.aliases,
                ),
            ],
            Vec::new(),
        )
        .unwrap();
        let core = self
            .core
            .foundation
            .import_core_inputs(&self.core.interface, &[])
            .unwrap();
        let input = OrdinarySources::try_new(&ordinary, core, &world).unwrap();
        let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
            .expect("core-closed dependency constants must inline in an ordinary consumer");
        inspect(output)
    }
}

#[test]
fn exact_alias_and_star_imports_inline_literals_with_split_origins() {
    let fixture = DependencyConstantFixture::new();
    let answer = exact_import(&["dependency", "api", "answer"]);
    let mut enabled = exact_import(&["dependency", "api", "enabled"]);
    let scoop_ast::ImportSyntax::Exact { alias, .. } = &mut enabled else {
        unreachable!("exact_import builds an exact import")
    };
    *alias = Some(scoop_ast::ImportAliasSyntax {
        as_keyword_span: sp(),
        name: ident("flag"),
        span: sp(),
    });
    let mut consumer = file(vec![
        fun_expr(
            "readAnswer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            var("answer"),
        ),
        fun_expr(
            "readFlag",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Boolean")),
            var("flag"),
        ),
        fun_expr(
            "readLabel",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            var("label"),
        ),
    ]);
    consumer.imports.append(&mut vec![
        answer,
        enabled,
        scoop_ast::ImportSyntax::Star {
            exposure: scoop_ast::ImportExposureSyntax::Local,
            namespace: qualified(&["dependency", "api"]),
            import_keyword_span: sp(),
            terminal_dot_span: sp(),
            star_span: sp(),
            span: sp(),
        },
    ]);
    let provider = fixture.coordinate.identity().unwrap();

    fixture.inspect(consumer, |output| {
        assert_eq!(output.imported_dependencies().callable_count(), 0);
        assert_eq!(output.imported_dependencies().constant_count(), 3);
        assert_eq!(output.concrete_dependency_witness_uses().len(), 3);
        assert!(
            output
                .concrete_dependency_witness_uses()
                .iter()
                .all(|use_| {
                    use_.role() == scoop_hir::ExternalHirBindingWitnessRole::ConcreteSelectedUse
                        && matches!(
                            use_.target(),
                            scoop_hir::ExternalHirTargetV1::Property(
                                scoop_identity::PropertyOwner::Property(_)
                            )
                        )
                })
        );
        assert!(
            output
                .output()
                .export
                .imported_dependency_callables
                .is_empty()
        );

        let module = output.output().export.module();
        let answer = function_result(module, "readAnswer");
        assert!(matches!(
            answer.kind,
            scoop_hir::ExprKind::IntegerLiteral(scoop_hir::HirIntegerConstant::Signed32(42))
        ));
        let flag = function_result(module, "readFlag");
        assert!(matches!(flag.kind, scoop_hir::ExprKind::BoolLiteral(true)));
        let label = function_result(module, "readLabel");
        assert!(matches!(
            &label.kind,
            scoop_hir::ExprKind::StringLiteral {
                value,
                owner: scoop_hir::StringConstantOwner::CurrentDefinition,
            } if value == "forty-two"
        ));

        for expression in [answer, flag, label] {
            let origin = expression.origin.concrete();
            assert_eq!(source_cone(module, origin.definition.file), provider);
            assert_eq!(source_cone(module, origin.evaluation.file), module.cone);
        }
    });
}

#[test]
fn imported_constant_folds_into_local_const_and_static_image_once() {
    let fixture = DependencyConstantFixture::new();
    let mut consumer = file(vec![
        const_property("copied", ty_named("Int"), var("answer")),
        stored_property("cached", ty_named("Int"), var("answer")),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "answer"]));

    fixture.inspect(consumer, |output| {
        assert_eq!(output.imported_dependencies().constant_count(), 1);
        let module = output.output().export.module();
        let copied = module
            .properties
            .iter()
            .find_map(|(_, property)| (property.name == "copied").then_some(property))
            .expect("the local const must be materialized");
        assert!(matches!(
            copied.representation,
            scoop_hir::PropertyRepresentation::Const {
                value: scoop_hir::ConstPropertyValue::Integer(
                    scoop_hir::HirIntegerConstant::Signed32(42)
                )
            }
        ));
        let cached = module
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == "cached").then_some(global))
            .expect("the static property must own local storage");
        assert!(matches!(
            cached.storage,
            scoop_hir::GlobalStorage::Managed {
                state: scoop_hir::HirStaticInitialState::EncodedStaticValue {
                    payload: scoop_hir::HirConstantImage::Integer(
                        scoop_hir::HirIntegerConstant::Signed32(42)
                    )
                }
            }
        ));
    });
}

fn const_property(name: &str, ty: TypeRef, expression: Expr) -> Decl {
    Decl::Global(scoop_ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: scoop_ast::VisibilitySyntax::Omitted,
        modifier: scoop_ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: scoop_ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    })
}

fn stored_property(name: &str, ty: TypeRef, expression: Expr) -> Decl {
    let Decl::Global(mut property) = const_property(name, ty, expression) else {
        unreachable!("const_property builds a global property")
    };
    property.body = scoop_ast::PropertyBodySyntax::Initializer {
        expression: match property.body {
            scoop_ast::PropertyBodySyntax::Const(expression) => expression,
            _ => unreachable!("const_property builds a const body"),
        },
        accessors: scoop_ast::AccessorSyntax::default(),
    };
    Decl::Global(property)
}

fn function_result<'a>(module: &'a scoop_hir::Module, name: &str) -> &'a scoop_hir::Expr {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("missing consumer function `{name}`"));
    let scoop_hir::FunctionKind::User(body) = &function.kind else {
        panic!("consumer functions have source bodies")
    };
    return_value(&body.statements)
}

fn source_cone(module: &scoop_hir::Module, file: u32) -> ConeIdentity {
    module.source_files[usize::try_from(file).unwrap()]
        .identity
        .cone()
}

fn qualified(parts: &[&str]) -> scoop_ast::QualifiedNameSyntax {
    let (first, rest) = parts.split_first().expect("qualified names are nonempty");
    scoop_ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| scoop_ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}
