use super::*;
mod surface;

fn foreign_fixture() -> Fixture {
    Fixture::new(Options {
        first_type_has_itable: true,
        first_itable_interface: Some(foreign_exact("Interface")),
        ..Options::default()
    })
}
