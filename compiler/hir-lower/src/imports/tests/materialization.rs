use super::*;

fn invalid_raw_string_property(name: &str) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: vec![ast::Annotation {
            name: ident("Global"),
            args: Vec::new(),
            span: sp(),
        }],
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("String"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(str_lit("invalid raw image")),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

#[test]
fn failed_property_materialization_does_not_stop_independent_body_diagnostics() {
    let errors = lower_sources(vec![file(vec![
        invalid_raw_string_property("broken"),
        fun("main", vec![stmt(call("missing", Vec::new()))]),
    ])])
    .expect_err("both the declaration and independent body use must be diagnosed");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("has no valid all-zero initial image")),
        "unexpected diagnostics: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("unknown function `missing`")),
        "body lowering stopped after the declaration error: {messages:?}"
    );
}

#[test]
fn unmaterialized_exact_value_blocks_core_fallback_without_becoming_a_target() {
    let producer = package(
        file(vec![
            fun("println", Vec::new()),
            invalid_raw_string_property("println"),
        ]),
        &["api"],
    );
    let mut consumer = file(vec![fun("main", vec![stmt(call("println", Vec::new()))])]);
    consumer.imports = vec![exact(&["api", "println"], None, false)];

    let errors = lower_sources(vec![producer, consumer])
        .expect_err("an unmaterialized exact target must not fall through to core");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("has no valid all-zero initial image")),
        "unexpected diagnostics: {messages:?}"
    );
    assert!(
        messages.contains(&"function `println` is not accessible here"),
        "the failed exact target silently downgraded to the core prelude: {messages:?}"
    );
}

#[test]
fn failed_variant_does_not_shift_a_later_source_variant_identity() {
    let errors = lower_sources(vec![file(vec![
        enum_decl(
            "Status",
            Vec::new(),
            vec![
                variant_positional("Broken", vec![ty_named("Missing")]),
                variant_unit("Good"),
            ],
        ),
        fun("main", vec![val("good", field(var("Status"), "Good"))]),
    ])])
    .expect_err("the invalid first variant field type must be diagnosed");
    assert_eq!(errors.len(), 1, "unexpected diagnostics: {errors:?}");
    assert_eq!(errors[0].message, "unknown type `Missing`");
}
