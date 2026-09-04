use super::super::*;

pub(super) fn coroutine_core_declarations() -> Vec<Decl> {
    let continuation = generic_interface_decl(
        "Continuation",
        vec!["T"],
        vec![
            bodyless_method(false, "resume", vec![("value", ty_named("T"))], None),
            bodyless_method(
                false,
                "resumeWithException",
                vec![("exception", ty_named("Throwable"))],
                None,
            ),
        ],
    );
    let task = generic_interface_decl(
        "SuspendTask",
        vec!["T"],
        vec![with_suspend(bodyless_method(
            false,
            "run",
            vec![],
            Some(ty_named("T")),
        ))],
    );
    let registration = generic_interface_decl(
        "SuspendRegistration",
        vec!["T"],
        vec![bodyless_method(
            false,
            "register",
            vec![(
                "continuation",
                ty_generic("Continuation", vec![ty_named("T")]),
            )],
            None,
        )],
    );
    let start = intrinsic_generic_fun(
        "startCoroutine",
        "coroutine_start",
        vec!["T"],
        vec![
            ("task", ty_generic("SuspendTask", vec![ty_named("T")])),
            (
                "completion",
                ty_generic("Continuation", vec![ty_named("T")]),
            ),
        ],
        None,
    );
    let Decl::Function(mut suspend) = intrinsic_generic_fun(
        "suspendCoroutine",
        "coroutine_suspend",
        vec!["T"],
        vec![(
            "registration",
            ty_generic("SuspendRegistration", vec![ty_named("T")]),
        )],
        Some(ty_named("T")),
    ) else {
        unreachable!("intrinsic_generic_fun always builds a function declaration")
    };
    suspend.is_suspend = true;

    vec![
        continuation,
        task,
        registration,
        start,
        Decl::Function(suspend),
    ]
}
