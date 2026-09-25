use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    CanonicalNominalSourceParameterProtocolsV1 as Table, NominalParameterBindingError as Error,
    NominalSourceParameterProtocolV1 as Record, SourceParameterContractError as ContractError,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

mod contracts;
mod inventories;
mod origins;
mod replay;
pub(super) mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/parameters-binding.scoop"
));

#[test]
fn complete_parameter_sources_bind_all_roles_from_restored_bytes() {
    for source in [
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/constructors.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/members-binding.scoop"
        )),
    ] {
        with_sources(source, |output, fixture, sources, core| {
            if source == SOURCE {
                assert_eq!(
                    super::super::source_nominal_parameters::contracts::verify(
                        &output.output().export,
                        &sources.protocols
                    ),
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-nominals/parameters-binding.snap"
                    )),
                );
            }
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let bound = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                assert_eq!(bound.provider(), members.provider());
                assert!(std::ptr::eq(bound.members(), members));
                assert!(std::ptr::eq(bound.constructors(), constructors));
                assert_eq!(bound.table(), &sources.protocols);
                for record in sources.protocols.records() {
                    assert_eq!(bound.protocol(record.owner()).unwrap(), record);
                    for (position, parameter) in record.parameters().iter().enumerate() {
                        assert_eq!(
                            bound.parameter(record.owner(), position as u32).unwrap(),
                            parameter
                        );
                    }
                    let Error::Position { owner, .. } = bound
                        .parameter(record.owner(), record.parameters().len() as u32)
                        .unwrap_err()
                    else {
                        panic!("missing parameter position");
                    };
                    assert_eq!(owner, record.owner());
                }
                assert!(
                    sources
                        .protocols
                        .records()
                        .iter()
                        .any(|r| r.parameters().is_empty())
                );
                if source == SOURCE {
                    assert!(
                        sources.protocols.records().iter().any(|r| matches!(
                            r.owner(),
                            CallableTemplateOrigin::GenericFunction(_)
                        ))
                    );
                    assert!(sources.protocols.records().iter().any(|r| matches!(
                        r.owner(),
                        CallableTemplateOrigin::VariantConstructor(_)
                    )));
                }
            });
        });
    }
}
