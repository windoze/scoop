use std::cmp::Ordering;

use scoop_identity::{
    CallableTemplateOrigin, OptionalSignatureType, PersistentFieldId,
    PersistentGeneratedCallableId, SignatureTypeKey,
};
use scoop_wire::{WireError, WirePath};

use crate::{
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableDeclarationV1,
    DefaultCallableRefV1, DefaultConstructorRefV1, DefaultEnumVariantRefV1, DefaultFieldRefV1,
    ExportDefaultCallableTargetV1,
};

mod views;
pub use views::compare_default_signature_reference_targets;

#[derive(Clone, Copy, Debug)]
pub enum CallableTargetView<'a> {
    Callable(&'a DefaultCallableRefV1),
    Bound(&'a DefaultBoundCallableRefV1),
    DerivedEquality(&'a SignatureTypeKey),
    LocalFunction(CallableTemplateOrigin),
    Lambda(PersistentGeneratedCallableId),
    AnonymousFunction(PersistentGeneratedCallableId),
    CallableReference(PersistentGeneratedCallableId),
    FunctionAddress(DefaultCallableDeclarationV1),
}

impl CallableTargetView<'_> {
    const fn tag(self) -> u8 {
        match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality(_) => 3,
            Self::LocalFunction(_) => 4,
            Self::Lambda(_) => 5,
            Self::AnonymousFunction(_) => 6,
            Self::CallableReference(_) => 7,
            Self::FunctionAddress(_) => 8,
        }
    }
}

pub(super) fn callable_target(
    declared: &ExportDefaultCallableTargetV1,
    actual: CallableTargetView<'_>,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let declared_tag = match declared {
        ExportDefaultCallableTargetV1::Callable(_) => 1,
        ExportDefaultCallableTargetV1::Bound(_) => 2,
        ExportDefaultCallableTargetV1::DerivedEquality { .. } => 3,
        ExportDefaultCallableTargetV1::LocalFunction { .. } => 4,
        ExportDefaultCallableTargetV1::Lambda { .. } => 5,
        ExportDefaultCallableTargetV1::AnonymousFunction { .. } => 6,
        ExportDefaultCallableTargetV1::CallableReference { .. } => 7,
        ExportDefaultCallableTargetV1::FunctionAddress { .. } => 8,
    };
    let ordering = declared_tag.cmp(&actual.tag());
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    match (declared, actual) {
        (ExportDefaultCallableTargetV1::Callable(left), CallableTargetView::Callable(right)) => {
            callable_ref(left, right, path)
        }
        (ExportDefaultCallableTargetV1::Bound(left), CallableTargetView::Bound(right)) => {
            bound_callable_ref(left, right, path)
        }
        (
            ExportDefaultCallableTargetV1::DerivedEquality { owner_type: left },
            CallableTargetView::DerivedEquality(right),
        ) => signature_type(left, right, path),
        (
            ExportDefaultCallableTargetV1::LocalFunction { declaration: left },
            CallableTargetView::LocalFunction(right),
        ) => Ok(left.cmp(&right)),
        (
            ExportDefaultCallableTargetV1::Lambda { body: left },
            CallableTargetView::Lambda(right),
        )
        | (
            ExportDefaultCallableTargetV1::AnonymousFunction { body: left },
            CallableTargetView::AnonymousFunction(right),
        )
        | (
            ExportDefaultCallableTargetV1::CallableReference { invoke: left },
            CallableTargetView::CallableReference(right),
        ) => Ok(left.cmp(&right)),
        (
            ExportDefaultCallableTargetV1::FunctionAddress { declaration: left },
            CallableTargetView::FunctionAddress(right),
        ) => Ok(left.cmp(&right)),
        _ => Ok(Ordering::Equal),
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ConstructorTargetView<'a> {
    Constructor(&'a DefaultConstructorRefV1),
    Variant(&'a DefaultEnumVariantRefV1),
}

pub(super) fn constructor_target(
    declared: &DefaultConstructorRefV1,
    actual: ConstructorTargetView<'_>,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    match actual {
        ConstructorTargetView::Constructor(actual) => constructor_ref(declared, actual, path),
        ConstructorTargetView::Variant(actual) => {
            let declared_tag = constructor_tag(declared);
            let ordering = declared_tag.cmp(&3);
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            let DefaultConstructorRefV1::Variant {
                declaration,
                owner_type,
            } = declared
            else {
                return Ok(Ordering::Equal);
            };
            let ordering = declaration.cmp(&actual.declaration());
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            signature_type(owner_type, actual.owner_type(), path)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FieldTargetView<'a> {
    Field(&'a DefaultFieldRefV1),
    Struct {
        declaration: PersistentFieldId,
        owner_type: &'a SignatureTypeKey,
    },
}

pub(super) fn field_target(
    declared: &DefaultFieldRefV1,
    actual: FieldTargetView<'_>,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    match actual {
        FieldTargetView::Field(actual) => field_ref(declared, actual, path),
        FieldTargetView::Struct {
            declaration: actual_declaration,
            owner_type: actual_owner,
        } => {
            let ordering = field_tag(declared).cmp(&1);
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            let DefaultFieldRefV1::Struct {
                declaration,
                owner_type,
            } = declared
            else {
                return Ok(Ordering::Equal);
            };
            let ordering = declaration.cmp(&actual_declaration);
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            signature_type(owner_type, actual_owner, path)
        }
    }
}

pub(super) fn signature_type(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    compare_type_tasks(TypeComparison::Type(left, right), path)
}

fn signature_types(
    left: &[SignatureTypeKey],
    right: &[SignatureTypeKey],

    path: &WirePath,
) -> Result<Ordering, WireError> {
    compare_type_tasks(TypeComparison::Sequence(left, right), path)
}

fn compare_type_tasks<'a>(
    initial: TypeComparison<'a>,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let mut pending = Vec::new();
    scoop_wire::allocation::try_reserve(&mut pending, 1, path)?;
    pending.push(initial);
    while let Some(task) = pending.pop() {
        match task {
            TypeComparison::Type(left, right) => {
                let ordering = signature_tag(left).cmp(&signature_tag(right));
                if ordering != Ordering::Equal {
                    return Ok(ordering);
                }
                match (left, right) {
                    (SignatureTypeKey::Nominal(left), SignatureTypeKey::Nominal(right)) => {
                        let ordering = left.cmp(right);
                        if ordering != Ordering::Equal {
                            return Ok(ordering);
                        }
                    }
                    (
                        SignatureTypeKey::NominalApplication {
                            origin: left_origin,
                            arguments: left_arguments,
                        },
                        SignatureTypeKey::NominalApplication {
                            origin: right_origin,
                            arguments: right_arguments,
                        },
                    ) => {
                        let ordering = left_origin.cmp(right_origin);
                        if ordering != Ordering::Equal {
                            return Ok(ordering);
                        }
                        push_task(
                            &mut pending,
                            TypeComparison::Sequence(
                                left_arguments.as_slice(),
                                right_arguments.as_slice(),
                            ),
                            path,
                        )?;
                    }
                    (SignatureTypeKey::Tuple(left), SignatureTypeKey::Tuple(right)) => {
                        push_task(
                            &mut pending,
                            TypeComparison::Sequence(left.as_slice(), right.as_slice()),
                            path,
                        )?;
                    }
                    (
                        SignatureTypeKey::Function {
                            effect: left_effect,
                            parameters: left_parameters,
                            result: left_result,
                        },
                        SignatureTypeKey::Function {
                            effect: right_effect,
                            parameters: right_parameters,
                            result: right_result,
                        },
                    ) => {
                        let ordering = left_effect.cmp(right_effect);
                        if ordering != Ordering::Equal {
                            return Ok(ordering);
                        }
                        push_task(
                            &mut pending,
                            TypeComparison::Type(left_result, right_result),
                            path,
                        )?;
                        push_task(
                            &mut pending,
                            TypeComparison::Sequence(left_parameters, right_parameters),
                            path,
                        )?;
                    }
                    (SignatureTypeKey::RawPointer(left), SignatureTypeKey::RawPointer(right)) => {
                        push_task(&mut pending, TypeComparison::Type(left, right), path)?;
                    }
                    (
                        SignatureTypeKey::NativeFunctionPointer {
                            calling_convention: left_calling_convention,
                            parameters: left_parameters,
                            result: left_result,
                        },
                        SignatureTypeKey::NativeFunctionPointer {
                            calling_convention: right_calling_convention,
                            parameters: right_parameters,
                            result: right_result,
                        },
                    ) => {
                        let ordering = left_calling_convention.cmp(right_calling_convention);
                        if ordering != Ordering::Equal {
                            return Ok(ordering);
                        }
                        push_task(
                            &mut pending,
                            TypeComparison::Type(left_result, right_result),
                            path,
                        )?;
                        push_task(
                            &mut pending,
                            TypeComparison::Sequence(left_parameters, right_parameters),
                            path,
                        )?;
                    }
                    (
                        SignatureTypeKey::Binder {
                            depth: left_depth,
                            index: left_index,
                        },
                        SignatureTypeKey::Binder {
                            depth: right_depth,
                            index: right_index,
                        },
                    ) => {
                        let ordering = left_depth
                            .cmp(right_depth)
                            .then_with(|| left_index.cmp(right_index));
                        if ordering != Ordering::Equal {
                            return Ok(ordering);
                        }
                    }
                    _ => return Ok(Ordering::Equal),
                }
            }
            TypeComparison::Sequence(left, right) => {
                push_task(
                    &mut pending,
                    TypeComparison::Length(left.len(), right.len()),
                    path,
                )?;
                let common = left.len().min(right.len());
                scoop_wire::allocation::try_reserve(&mut pending, common, path)?;
                for index in (0..common).rev() {
                    pending.push(TypeComparison::Type(&left[index], &right[index]));
                }
            }
            TypeComparison::Length(left, right) => {
                let ordering = left.cmp(&right);
                if ordering != Ordering::Equal {
                    return Ok(ordering);
                }
            }
        }
    }
    Ok(Ordering::Equal)
}

fn push_task<'a>(
    pending: &mut Vec<TypeComparison<'a>>,
    task: TypeComparison<'a>,

    path: &WirePath,
) -> Result<(), WireError> {
    scoop_wire::allocation::try_reserve(pending, 1, path)?;
    pending.push(task);
    Ok(())
}

#[derive(Clone, Copy)]
enum TypeComparison<'a> {
    Type(&'a SignatureTypeKey, &'a SignatureTypeKey),
    Sequence(&'a [SignatureTypeKey], &'a [SignatureTypeKey]),
    Length(usize, usize),
}

fn callable_ref(
    left: &DefaultCallableRefV1,
    right: &DefaultCallableRefV1,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let ordering = left.declaration().cmp(&right.declaration());
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    let ordering = optional_type(left.owner(), right.owner(), path)?;
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    signature_types(left.type_arguments(), right.type_arguments(), path)
}

fn optional_type(
    left: &OptionalSignatureType,
    right: &OptionalSignatureType,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    match (left, right) {
        (OptionalSignatureType::Absent, OptionalSignatureType::Absent) => Ok(Ordering::Equal),
        (OptionalSignatureType::Absent, OptionalSignatureType::Present(_)) => Ok(Ordering::Less),
        (OptionalSignatureType::Present(_), OptionalSignatureType::Absent) => Ok(Ordering::Greater),
        (OptionalSignatureType::Present(left), OptionalSignatureType::Present(right)) => {
            signature_type(left, right, path)
        }
    }
}

fn bound_callable_ref(
    left: &DefaultBoundCallableRefV1,
    right: &DefaultBoundCallableRefV1,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let ordering = signature_type(left.receiver_type(), right.receiver_type(), path)?;
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    let ordering = bound_source(left.source(), right.source(), path)?;
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    signature_type(
        left.instantiated_signature(),
        right.instantiated_signature(),
        path,
    )
}

fn bound_source(
    left: &DefaultBoundCallableSourceV1,
    right: &DefaultBoundCallableSourceV1,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let left_tag = match left {
        DefaultBoundCallableSourceV1::Class { .. } => 1,
        DefaultBoundCallableSourceV1::Interface { .. } => 2,
    };
    let right_tag = match right {
        DefaultBoundCallableSourceV1::Class { .. } => 1,
        DefaultBoundCallableSourceV1::Interface { .. } => 2,
    };
    let ordering = left_tag.cmp(&right_tag);
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    match (left, right) {
        (
            DefaultBoundCallableSourceV1::Class {
                bound: left_bound,
                callable: left_callable,
            },
            DefaultBoundCallableSourceV1::Class {
                bound: right_bound,
                callable: right_callable,
            },
        ) => {
            let ordering = signature_type(left_bound, right_bound, path)?;
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            callable_ref(left_callable, right_callable, path)
        }
        (
            DefaultBoundCallableSourceV1::Interface {
                bound: left_bound,
                member: left_member,
            },
            DefaultBoundCallableSourceV1::Interface {
                bound: right_bound,
                member: right_member,
            },
        ) => {
            let ordering = signature_type(left_bound, right_bound, path)?;
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            Ok(left_member.cmp(right_member))
        }
        _ => Ok(Ordering::Equal),
    }
}

fn constructor_ref(
    left: &DefaultConstructorRefV1,
    right: &DefaultConstructorRefV1,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let ordering = constructor_tag(left).cmp(&constructor_tag(right));
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    let ordering = match (left, right) {
        (
            DefaultConstructorRefV1::Struct {
                declaration: left, ..
            },
            DefaultConstructorRefV1::Struct {
                declaration: right, ..
            },
        ) => left.cmp(right),
        (
            DefaultConstructorRefV1::Class {
                declaration: left, ..
            },
            DefaultConstructorRefV1::Class {
                declaration: right, ..
            },
        ) => left.cmp(right),
        (
            DefaultConstructorRefV1::Variant {
                declaration: left, ..
            },
            DefaultConstructorRefV1::Variant {
                declaration: right, ..
            },
        ) => left.cmp(right),
        _ => Ordering::Equal,
    };
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    signature_type(left.owner_type(), right.owner_type(), path)
}

const fn constructor_tag(value: &DefaultConstructorRefV1) -> u8 {
    match value {
        DefaultConstructorRefV1::Struct { .. } => 1,
        DefaultConstructorRefV1::Class { .. } => 2,
        DefaultConstructorRefV1::Variant { .. } => 3,
    }
}

fn field_ref(
    left: &DefaultFieldRefV1,
    right: &DefaultFieldRefV1,

    path: &WirePath,
) -> Result<Ordering, WireError> {
    let ordering = field_tag(left).cmp(&field_tag(right));
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    match (left, right) {
        (
            DefaultFieldRefV1::Struct {
                declaration: left_declaration,
                owner_type: left_owner,
            },
            DefaultFieldRefV1::Struct {
                declaration: right_declaration,
                owner_type: right_owner,
            },
        )
        | (
            DefaultFieldRefV1::Class {
                declaration: left_declaration,
                owner_type: left_owner,
            },
            DefaultFieldRefV1::Class {
                declaration: right_declaration,
                owner_type: right_owner,
            },
        ) => {
            let ordering = left_declaration.cmp(right_declaration);
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
            signature_type(left_owner, right_owner, path)
        }
        (
            DefaultFieldRefV1::Tuple {
                declaration_index: left,
            },
            DefaultFieldRefV1::Tuple {
                declaration_index: right,
            },
        ) => Ok(left.cmp(right)),
        _ => Ok(Ordering::Equal),
    }
}

const fn field_tag(value: &DefaultFieldRefV1) -> u8 {
    match value {
        DefaultFieldRefV1::Struct { .. } => 1,
        DefaultFieldRefV1::Tuple { .. } => 2,
        DefaultFieldRefV1::Class { .. } => 3,
    }
}

const fn signature_tag(value: &SignatureTypeKey) -> u8 {
    match value {
        SignatureTypeKey::Nominal(_) => 1,
        SignatureTypeKey::NominalApplication { .. } => 2,
        SignatureTypeKey::Tuple(_) => 3,
        SignatureTypeKey::Function { .. } => 4,
        SignatureTypeKey::RawPointer(_) => 5,
        SignatureTypeKey::NativeFunctionPointer { .. } => 6,
        SignatureTypeKey::Binder { .. } => 7,
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{CallingConvention, Effect, NonEmptyVec};

    use super::*;
    use crate::cross_cone_interface::default_templates::body::expression_test_support::Fixture;

    #[test]
    fn signature_order_matches_the_canonical_derived_order() {
        let fixture = Fixture::new();
        let nominal = SignatureTypeKey::Nominal(fixture.type_id);
        let samples = vec![
            nominal.clone(),
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(binder(0), [binder(1)])),
            SignatureTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![nominal.clone(), binder(0)],
                result: Box::new(binder(1)),
            },
            SignatureTypeKey::RawPointer(Box::new(nominal.clone())),
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![binder(0)],
                result: Box::new(nominal),
            },
            binder(0),
            binder(1),
        ];

        for left in &samples {
            for right in &samples {
                assert_eq!(
                    signature_type(left, right, &WirePath::root(),),
                    Ok(left.cmp(right))
                );
            }
        }
    }

    const fn binder(index: u32) -> SignatureTypeKey {
        SignatureTypeKey::Binder { depth: 0, index }
    }
}
