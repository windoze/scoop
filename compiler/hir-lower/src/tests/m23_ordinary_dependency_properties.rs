use scoop_identity::ConeCoordinate;

use super::m23_ordinary_core_only::support::{TrustedCoreFixture, parsed_ordinary, trusted_core};
use super::m23_ordinary_dependencies::support::{
    certificate, empty_alias_expansions, exact_import, project_dependency_without_default_roles,
};
use super::{
    Decl, Expr, Statement, TypeRef, assign, assign_field, block, bool_lit, extension_expr, field,
    file, fun, fun_expr, ident, int_lit, make_core_public, method_call, sp, this_expr, tuple_lit,
    ty_named, ty_tuple, var,
};
use crate::{CurrentConeSources, lower_current_cone};

mod extension;

struct DependencyPropertyFixture {
    core: TrustedCoreFixture,
    coordinate: ConeCoordinate,
    foundation: scoop_hir::ImportedHirFoundation,
    interface: scoop_hir::CrossConeHirInterfaceSectionV1,
    aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
}

impl DependencyPropertyFixture {
    fn new(properties: Vec<Decl>) -> Self {
        Self::with_core_types(properties, &["Int"])
    }

    fn with_core_types(properties: Vec<Decl>, core_types: &[&str]) -> Self {
        let mut core = trusted_core();
        let coordinate = ConeCoordinate::new("test", "property-provider", "1.0.0").unwrap();
        let mut provider = file(properties);
        make_core_public(&mut provider);
        provider.package = scoop_ast::PackageSyntax::QualifiedPackage {
            package_keyword_span: sp(),
            path: qualified(&["dependency", "api"]),
            span: sp(),
        };
        let (foundation, interface) =
            project_dependency_without_default_roles(&core, &coordinate, provider, core_types);
        let foundation = core.import_dependency_foundation(&coordinate, &foundation, 57);
        Self {
            core,
            coordinate,
            foundation,
            interface,
            aliases: empty_alias_expansions(),
        }
    }

    fn inspect<R>(
        &self,
        consumer: scoop_ast::SourceFile,
        inspect: impl FnOnce(Result<scoop_hir::DependencyHirOutput, Vec<scoop_ast::Diagnostic>>) -> R,
    ) -> R {
        let ordinary = parsed_ordinary(consumer);
        let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
            ordinary.cone(),
            vec![
                self.core.provider(),
                scoop_hir::DirectImportedProviderInput::from_validated(
                    certificate(&self.coordinate, 57),
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
            .import_core_inputs(&self.core.interface)
            .unwrap();
        let input = CurrentConeSources::try_new(&ordinary, core, &world).unwrap();
        inspect(lower_current_cone(
            scoop_identity::RequestedConeKind::Library,
            &input,
        ))
    }
}

#[test]
fn top_level_getter_setter_and_update_use_two_dependency_accessors() {
    let fixture = DependencyPropertyFixture::new(vec![computed_property(
        "counter",
        ty_named("Int"),
        int_lit(1),
        Some(Vec::new()),
    )]);
    let mut import = exact_import(&["dependency", "api", "counter"]);
    let scoop_ast::ImportSyntax::Exact { alias, .. } = &mut import else {
        unreachable!("exact_import builds an exact import")
    };
    *alias = Some(scoop_ast::ImportAliasSyntax {
        as_keyword_span: sp(),
        name: ident("state"),
        span: sp(),
    });
    let update = Expr::Update {
        place: scoop_ast::PlaceExpr::Name(ident("state")),
        op: scoop_ast::UpdateOp::Increment,
        notation: scoop_ast::UpdateNotation::Postfix,
        span: sp(),
    };
    let mut consumer = file(vec![
        integer_increment_extension(),
        fun_expr(
            "read",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            var("state"),
        ),
        fun("write", vec![assign("state", int_lit(9))]),
        fun_expr(
            "bump",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            update,
        ),
    ]);
    consumer.imports.push(import);

    fixture.inspect(consumer, |output| {
        let output = output.expect("core-closed dependency property accessors must lower");
        assert_eq!(output.imported_dependencies().callable_count(), 2);
        assert_eq!(output.imported_dependencies().constant_count(), 0);
        assert_eq!(
            output.output().export.imported_dependency_callables.len(),
            2
        );
        assert_eq!(output.output().local.imported_dependency_callables.len(), 2);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("ImportedDependencyCall").count(), 4, "{dump}");
        assert_eq!(
            output
                .imported_dependencies()
                .callables()
                .filter(|callable| {
                    matches!(
                        callable.capability().declaration(),
                        scoop_identity::DependencyCallableDeclarationId::PropertyAccessor(_)
                    )
                })
                .count(),
            2
        );
    });
}

fn integer_increment_extension() -> Decl {
    let Decl::Function(mut function) = extension_expr(
        ty_named("Int"),
        "inc",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        this_expr(),
    ) else {
        unreachable!("extension_expr builds a function")
    };
    function.operator = Some(scoop_ast::OperatorModifier { span: sp() });
    Decl::Function(function)
}

#[test]
fn read_only_dependency_property_rejects_assignment_before_accessor_selection() {
    let fixture = DependencyPropertyFixture::new(vec![computed_property(
        "answer",
        ty_named("Int"),
        int_lit(42),
        None,
    )]);
    let mut consumer = file(vec![fun("write", vec![assign("answer", int_lit(9))])]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "answer"]));

    fixture.inspect(consumer, |output| {
        let diagnostics = match output {
            Ok(_) => panic!("a read-only dependency property cannot be assigned"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot assign to immutable property `answer`")
        }));
    });
}

#[test]
fn restricted_dependency_setter_is_not_callable() {
    let mut declaration = computed_property("state", ty_named("Int"), int_lit(1), Some(Vec::new()));
    let Decl::Global(property) = &mut declaration else {
        unreachable!("computed_property builds a global property")
    };
    let scoop_ast::PropertyBodySyntax::Computed(accessors) = &mut property.body else {
        unreachable!("computed_property builds accessors")
    };
    accessors
        .setter
        .as_mut()
        .expect("the fixture has a setter")
        .visibility = scoop_ast::SetterVisibilitySyntax::Explicit {
        visibility: scoop_ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let fixture = DependencyPropertyFixture::new(vec![declaration]);
    let mut consumer = file(vec![fun("write", vec![assign("state", int_lit(9))])]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "state"]));

    fixture.inspect(consumer, |output| {
        let diagnostics = match output {
            Ok(_) => panic!("a restricted dependency setter cannot be called"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("setter of property `state` is not accessible")
        }));
    });
}

#[test]
fn structural_dependency_property_reports_the_layout_capability_gate() {
    let fixture = DependencyPropertyFixture::new(vec![computed_property(
        "pair",
        ty_tuple(vec![ty_named("Int"), ty_named("Int")]),
        tuple_lit(vec![int_lit(1), int_lit(2)]),
        None,
    )]);
    let mut consumer = file(vec![fun_expr(
        "read",
        Vec::new(),
        Vec::new(),
        Some(ty_tuple(vec![ty_named("Int"), ty_named("Int")])),
        var("pair"),
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "pair"]));

    fixture.inspect(consumer, |output| {
        let diagnostics = match output {
            Ok(_) => panic!("a tuple-valued dependency property needs M23-6 layout capability"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED")
        }));
    });
}

fn computed_property(
    name: &str,
    ty: TypeRef,
    getter: Expr,
    setter: Option<Vec<Statement>>,
) -> Decl {
    Decl::Global(scoop_ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: scoop_ast::VisibilitySyntax::Omitted,
        modifier: scoop_ast::MethodModifier::Final,
        is_override: false,
        mutable: setter.is_some(),
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: scoop_ast::PropertyBodySyntax::Computed(scoop_ast::AccessorSyntax {
            getter: Some(scoop_ast::GetterDecl {
                annotations: Vec::new(),
                body: scoop_ast::AccessorBodySyntax::Expr(Box::new(getter)),
                span: sp(),
            }),
            setter: setter.map(|statements| scoop_ast::SetterDecl {
                annotations: Vec::new(),
                visibility: scoop_ast::SetterVisibilitySyntax::Inherited,
                parameter: scoop_ast::SetterParameterSyntax::Default { span: sp() },
                body: scoop_ast::AccessorBodySyntax::Block(block(statements)),
                span: sp(),
            }),
        }),
        span: sp(),
    })
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
