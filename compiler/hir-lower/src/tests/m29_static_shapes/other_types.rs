use super::*;

#[test]
fn static_shapes_distinguish_intrinsics_structural_types_and_pointers() {
    with_shapes(
        "package shapes\npublic struct Marker()",
        r#"
            public fun values(value: (Unit, Boolean, Int8, UInt8, Int16, UInt16, Int, UInt, Long, ULong, Char, String, Array<Int>, MutableArray<String>, Ptr<Int>, FunPtr<(Int) -> Int>, (Int) -> String)): (Unit, Boolean, Int8, UInt8, Int16, UInt16, Int, UInt, Long, ULong, Char, String, Array<Int>, MutableArray<String>, Ptr<Int>, FunPtr<(Int) -> Int>, (Int) -> String) = value
        "#,
        |module, world, _| {
            let ty = module
                .functions
                .values()
                .find(|function| function.name == "values")
                .unwrap()
                .return_ty;
            let hir::StaticTypeShape::Tuple(elements) = module.static_type_shape(ty, &[]).unwrap()
            else {
                panic!("tuple shape")
            };
            assert!(matches!(
                module.static_type_shape(elements[0], &[]).unwrap(),
                hir::StaticTypeShape::Unit
            ));
            for element in &elements[1..14] {
                let hir::StaticTypeShape::Nominal(shape) =
                    module.static_type_shape(*element, &[]).unwrap()
                else {
                    panic!("intrinsic nominal")
                };
                assert!(matches!(shape.kind(), hir::StaticNominalKind::Intrinsic(_)));
                assert!(shape.primary_constructor(world).is_none());
                assert_eq!(
                    shape.fields().len(),
                    0,
                    "an intrinsic is not an empty record"
                );
                for interface in shape.interfaces(world) {
                    interface.signature(module, &[]).unwrap();
                }
            }
            let hir::StaticTypeShape::Pointer(pointee) =
                module.static_type_shape(elements[14], &[]).unwrap()
            else {
                panic!("pointer shape")
            };
            assert_eq!(pointee, elements[6]);
            let hir::StaticTypeShape::NativeFunctionPointer(function) =
                module.static_type_shape(elements[15], &[]).unwrap()
            else {
                panic!("native function pointer")
            };
            assert_eq!(function.parameter_types, vec![elements[6]]);
            let hir::StaticTypeShape::Function(function) =
                module.static_type_shape(elements[16], &[]).unwrap()
            else {
                panic!("managed function")
            };
            assert_eq!(function.return_type, elements[11]);
            let dump = hir::dump_static_shapes(module, world);
            assert!(dump.contains("intrinsic core_array Array<Int>"));
            assert!(dump.contains("native-function-pointer FunPtr<(Int) -> Int>"));
            assert!(dump.contains("element _1: Unit"));
        },
    );
}

#[test]
fn static_shapes_keep_interface_properties_delegates_and_recursive_references() {
    with_shapes(
        r#"
            package shapes
            public annotation class Label(val text: String)
            public interface Parent<T> { public val value: T }
            public interface View<T> : Parent<T> { @Label("label") public val label: String }
            public class Slot<T> public constructor(private val value: T) {
                public operator fun getValue(thisRef: Holder<T>): T = value
            }
            public class Holder<T> public constructor(value: T) {
                @Label("stored") public val stored: T by Slot<T>(value)
            }
            public class Node public constructor(public val next: Option<Node>)
            public struct Restricted<T : ToString>(val value: T)
        "#,
        r#"
            import shapes.View
            import shapes.Holder
            import shapes.Node
            import shapes.Restricted
            public fun view(value: View<Int>): View<Int> = value
            public fun holder(value: Holder<String>): Holder<String> = value
            public fun node(value: Node): Node = value
            public fun restricted(value: Restricted<Int>): Restricted<Int> = value
        "#,
        |module, world, _| {
            let view = nominal(module, "view");
            assert_eq!(view.kind(), hir::StaticNominalKind::Interface);
            assert_eq!(view.fields().len(), 0);
            assert_eq!(view.interfaces(world).len(), 1);
            let properties = view.properties(world);
            assert_eq!(
                properties.len(),
                1,
                "inherited properties stay on their declaring interface"
            );
            assert_eq!(
                annotation_text(properties[0].annotations(world), module, world),
                "label"
            );
            assert!(properties[0].field_storage().is_none());
            let holder = nominal(module, "holder");
            let properties = holder.properties(world);
            assert_eq!(properties.len(), 1);
            let (field, storage) = properties[0].field_storage().unwrap();
            assert_eq!(
                storage,
                hir::NominalFieldStorage::PropertyDelegate(properties[0].identity())
            );
            assert_eq!(
                holder.fields().next().unwrap().identity(),
                hir::StaticFieldIdentity::Field(field)
            );
            assert_eq!(
                properties[0].value_type(&[]).unwrap(),
                signature(module, holder.arguments()[0])
            );
            assert_eq!(
                annotation_text(properties[0].annotations(world), module, world),
                "stored"
            );
            let node = nominal(module, "node");
            let SignatureTypeKey::NominalApplication { arguments, .. } = node
                .fields()
                .next()
                .unwrap()
                .value_type()
                .signature(module, &[])
                .unwrap()
            else {
                panic!("recursive option")
            };
            assert_eq!(
                arguments.as_slice(),
                &[signature(module, node.application_type())]
            );
            let restricted = nominal(module, "restricted");
            assert_eq!(
                restricted.parameters()[0]
                    .nominal_bounds_in_source_order()
                    .len(),
                1
            );
            let dump = hir::dump_static_shapes(module, world);
            assert!(dump.contains("parameter T: ToString"));
            assert!(dump.contains("property stored: String"));
            assert!(dump.contains("@Label"));
        },
    );
}
