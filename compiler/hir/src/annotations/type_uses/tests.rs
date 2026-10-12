use super::*;
use scoop_identity::{CallingConvention, Effect, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

#[test]
fn type_annotation_paths_and_sites_use_distinct_wire_tags() {
    for step in [
        TypeUseStep::Argument(4),
        TypeUseStep::TupleElement(4),
        TypeUseStep::FunctionReceiver,
        TypeUseStep::FunctionParameter(4),
        TypeUseStep::FunctionResult,
        TypeUseStep::PointerPointee,
        TypeUseStep::FunctionPointee,
    ] {
        assert_eq!(
            decode_canonical::<TypeUseStep>(&encode(&step).unwrap()).unwrap(),
            step
        );
    }
    for slot in [
        AnnotationTypeSlot::Value,
        AnnotationTypeSlot::Result,
        AnnotationTypeSlot::Supertype(2),
        AnnotationTypeSlot::Bound {
            parameter: 1,
            index: 2,
        },
        AnnotationTypeSlot::AnnotationParameter(2),
    ] {
        assert_eq!(
            decode_canonical::<AnnotationTypeSlot>(&encode(&slot).unwrap()).unwrap(),
            slot
        );
    }
    for bytes in [&[0x81, 1][..], &[0x82, 3, 0][..], &[0x81, 8][..]] {
        assert!(decode_canonical::<TypeUseStep>(bytes).is_err());
    }
}

#[test]
fn type_annotation_paths_distinguish_native_function_roots_and_function_children() {
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let native: AnnotationParameterType<_> = SignatureTypeKey::NativeFunctionPointer {
        calling_convention: CallingConvention::C,
        parameters: vec![binder.clone()],
        result: Box::new(binder.clone()),
    }
    .into();
    for path in [
        vec![],
        vec![TypeUseStep::FunctionPointee],
        vec![
            TypeUseStep::FunctionPointee,
            TypeUseStep::FunctionParameter(0),
        ],
        vec![TypeUseStep::FunctionPointee, TypeUseStep::FunctionResult],
    ] {
        assert!(type_annotation_path_exists(&native, &path));
    }
    for path in [
        vec![TypeUseStep::FunctionResult],
        vec![TypeUseStep::Argument(0)],
        vec![TypeUseStep::FunctionPointee, TypeUseStep::FunctionPointee],
        vec![
            TypeUseStep::FunctionPointee,
            TypeUseStep::FunctionParameter(1),
        ],
    ] {
        assert!(!type_annotation_path_exists(&native, &path));
    }
    let function = SignatureTypeKey::Function {
        effect: Effect::Suspend,
        receiver: Some(Box::new(binder.clone())),
        parameters: vec![binder.clone()],
        result: Box::new(binder),
    }
    .into();
    assert!(type_annotation_path_exists(
        &function,
        &[TypeUseStep::FunctionReceiver]
    ));
    assert!(!type_annotation_path_exists(
        &function,
        &[TypeUseStep::FunctionPointee]
    ));
}
