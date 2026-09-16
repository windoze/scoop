use scoop_identity::LocalValueSelector;
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn producer_canonicalizes_a_dense_parameter_prefix() {
    let first = parameter(0);
    let second = parameter(1);
    let list =
        CanonicalTemplateValueParametersV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert_eq!(list.parameters(), &[first.clone(), second]);
    assert_eq!(list.len_u32(), 2);
    assert!(!list.is_empty());
    assert_eq!(list.get(0), Some(&first));
    assert_eq!(list.get(2), None);
}

#[test]
fn producer_rejects_selector_mismatches_duplicates_and_gaps() {
    assert_eq!(
        TemplateValueParameterV1::try_new(0, parameter_selector(1)),
        Err(TemplateValueParameterBuildError::LocalMismatch {
            position: 0,
            actual: parameter_selector(1),
        })
    );

    assert_eq!(
        CanonicalTemplateValueParametersV1::try_new(vec![parameter(0), parameter(0)]),
        Err(TemplateValueParameterListBuildError::UnexpectedPosition {
            index: 1,
            expected: 1,
            actual: 0,
        })
    );
    assert_eq!(
        CanonicalTemplateValueParametersV1::try_new(vec![parameter(1)]),
        Err(TemplateValueParameterListBuildError::UnexpectedPosition {
            index: 0,
            expected: 0,
            actual: 1,
        })
    );
}

#[test]
fn value_parameters_have_fixed_indexed_wire_and_resolve() {
    let expected =
        CanonicalTemplateValueParametersV1::try_new(vec![parameter(0), parameter(1)]).unwrap();
    let mut locals = LocalResolver::new(vec![
        LocalValueSelector::This,
        parameter_selector(0),
        parameter_selector(1),
    ]);

    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    assert_eq!(hex(&bytes), "82a201000201a201010202");

    let decoded: DecodedCanonicalTemplateValueParametersV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut locals).unwrap(), expected);
}

#[test]
fn reader_rejects_noncanonical_positions_and_selector_mismatches() {
    let noncanonical = DecodedCanonicalTemplateValueParametersV1 {
        parameters: vec![
            DecodedTemplateValueParameterV1 {
                position: 1,
                local_index: 2,
            },
            DecodedTemplateValueParameterV1 {
                position: 0,
                local_index: 1,
            },
        ],
    };
    let mut locals = LocalResolver::new(vec![
        LocalValueSelector::This,
        parameter_selector(0),
        parameter_selector(1),
    ]);
    assert_eq!(
        noncanonical.resolve(&mut locals),
        Err(
            TemplateValueParameterListValidationError::UnexpectedPosition {
                index: 0,
                expected: 0,
                actual: 1,
            }
        )
    );

    let mismatch = DecodedCanonicalTemplateValueParametersV1 {
        parameters: vec![DecodedTemplateValueParameterV1 {
            position: 0,
            local_index: 2,
        }],
    };
    assert_eq!(
        mismatch.resolve(&mut locals),
        Err(TemplateValueParameterListValidationError::Parameter {
            index: 0,
            error: TemplateValueParameterResolutionError::Record(
                TemplateValueParameterBuildError::LocalMismatch {
                    position: 0,
                    actual: parameter_selector(1),
                }
            ),
        })
    );
}

#[test]
fn local_lookup_failures_preserve_parameter_positions() {
    let decoded = DecodedCanonicalTemplateValueParametersV1 {
        parameters: vec![DecodedTemplateValueParameterV1 {
            position: 0,
            local_index: 7,
        }],
    };
    let mut missing = LocalResolver::new(Vec::new());
    assert_eq!(
        decoded.resolve(&mut missing),
        Err(TemplateValueParameterListValidationError::Parameter {
            index: 0,
            error: TemplateValueParameterResolutionError::Local(LocalError::MissingIndex(7)),
        })
    );

    let list = CanonicalTemplateValueParametersV1::try_new(vec![parameter(0)]).unwrap();
    assert_eq!(
        list.index_locals(&mut missing).unwrap_err(),
        TemplateValueParameterListIndexError::Parameter {
            index: 0,
            error: TemplateValueParameterIndexError::Local(LocalError::MissingSelector(
                parameter_selector(0)
            )),
        }
    );
}

#[test]
fn decoder_rejects_non_exact_records_and_u32_overflow() {
    let extra_field = decode_canonical::<DecodedTemplateValueParameterV1>(
        &[0xa3, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        extra_field.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 3,
        }
    );

    let overflow = decode_canonical::<DecodedTemplateValueParameterV1>(
        &[
            0xa2, 0x01, 0x1b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
        ],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(overflow.kind(), &WireErrorKind::IntegerOutOfRange);
}

fn parameter(position: u32) -> TemplateValueParameterV1 {
    TemplateValueParameterV1::try_new(position, parameter_selector(position)).unwrap()
}

fn parameter_selector(position: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter {
        declaration_index: position,
    }
}

struct LocalResolver {
    selectors: Vec<LocalValueSelector>,
}

impl LocalResolver {
    const fn new(selectors: Vec<LocalValueSelector>) -> Self {
        Self { selectors }
    }
}

impl TemplateLocalSelectorResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.selectors.get(index))
            .cloned()
            .ok_or(LocalError::MissingIndex(index))
    }
}

impl TemplateLocalIndexResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.selectors
            .iter()
            .position(|candidate| candidate == selector)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| LocalError::MissingSelector(selector.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LocalError {
    MissingIndex(u32),
    MissingSelector(LocalValueSelector),
}

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LocalError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
