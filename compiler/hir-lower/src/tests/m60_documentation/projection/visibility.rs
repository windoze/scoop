use super::*;

#[test]
fn m60_documentation_projection_filters_effective_visibility_and_protected_members() {
    let input = project(
        &lower_documented(
            r#"
        internal class Internal {
            public fun leak(): Int = 1
        }
        public open class Open {
            protected fun extensionPoint(): Int = 1
            private fun detail(): Int = 2
            public var state: Int
                get() = 1
                private set(next) {}
        }
        public class Closed {
            protected fun unreachable(): Int = 1
        }
        public sealed class Sealed {
            protected fun sealedOnly(): Int = 1
        }
        public class Child : Sealed()
        public sealed class Family {
            protected fun reachableThroughChild(): Int = 1
        }
        public open class ExtensionChild : Family()
        "#,
        )
        .unwrap(),
    );
    let names = input
        .visible_items(false)
        .into_iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"extensionPoint"), "{names:?}");
    assert!(names.contains(&"reachableThroughChild"), "{names:?}");
    for absent in [
        "Internal",
        "leak",
        "detail",
        "unreachable",
        "sealedOnly",
        "set",
    ] {
        assert!(!names.contains(&absent), "{absent}: {names:?}");
    }
    let private = input
        .visible_items(true)
        .into_iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    for present in [
        "Internal",
        "leak",
        "detail",
        "unreachable",
        "sealedOnly",
        "set",
    ] {
        assert!(private.contains(&present), "{present}: {private:?}");
    }
    let state = item(&input, ApiKind::Property, "state")
        .property
        .as_ref()
        .unwrap();
    assert_eq!(state.getter.visibility, Visibility::Public);
    assert_eq!(
        state.setter.as_ref().unwrap().visibility,
        Visibility::Private
    );
}

#[test]
fn m60_documentation_projection_executable_defaults_to_nonlocal_private_items() {
    let source = "private fun detail() { fun local() {} }\nfun main() { detail() }";
    let output = lower_documented(source).unwrap();
    let output = output.output();
    let input = scoop_doc::project_cone(
        &output.export,
        output.documentation.as_ref().unwrap(),
        &scoop_identity::ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap(),
        scoop_identity::RequestedConeKind::Executable,
    )
    .unwrap();
    assert!(
        input
            .visible_items(false)
            .iter()
            .any(|item| item.name == "detail")
    );
    assert!(!input.items.iter().any(|item| item.name == "local"));
}
