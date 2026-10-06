use super::*;
use scoop_identity::SignatureTypeKey;
use std::fmt::Write;

mod members;
mod names;
mod selection;

/// The ordinary HIR dump includes source shapes for closed types and nominal
/// declarations. Open fields stay in their declaration's actual binder scope.
pub fn dump_static_shapes<'a>(module: &'a Module, world: &ImportedSemanticWorld<'a>) -> String {
    let mut dump = ShapeDump {
        module,
        world,
        out: String::new(),
    };
    let mapper = HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(module));
    for (ty, parameters) in selection::types(module) {
        let binders = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| HirSignatureBinder {
                parameter: parameter.id,
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let signature = match mapper.map(ty, &binders) {
            Ok(signature) => signature,
            Err(HirSignatureTypeMappingError::GeneratedNominal) => continue,
            Err(error) => panic!("complete source shape has an invalid signature: {error}"),
        };
        let name = dump.type_name(&signature, parameters);
        let shape = module
            .static_type_shape(ty, parameters)
            .expect("selected shape has its binder scope");
        match shape {
            StaticTypeShape::Nominal(shape) => dump.nominal(shape, &name, &binders, parameters),
            StaticTypeShape::Tuple(elements) => {
                writeln!(dump.out, "tuple {name}").unwrap();
                for (index, element) in elements.iter().enumerate() {
                    let element =
                        dump.type_name(&mapper.map(*element, &binders).unwrap(), parameters);
                    writeln!(dump.out, "  element _{}: {element}", index + 1).unwrap();
                }
            }
            StaticTypeShape::Unit => dump.out.push_str("unit Unit\n"),
            StaticTypeShape::Any => dump.out.push_str("any Any\n"),
            StaticTypeShape::Function(_) => writeln!(dump.out, "function {name}").unwrap(),
            StaticTypeShape::Pointer(_) => writeln!(dump.out, "pointer {name}").unwrap(),
            StaticTypeShape::NativeFunctionPointer(_) => {
                writeln!(dump.out, "native-function-pointer {name}").unwrap()
            }
            StaticTypeShape::Parameter(_) => {
                unreachable!("open parameters are printed with their nominal declaration")
            }
        }
    }
    dump.out
}

struct ShapeDump<'a, 'world> {
    module: &'a Module,
    world: &'world ImportedSemanticWorld<'a>,
    out: String,
}

impl<'a> ShapeDump<'a, '_> {
    fn nominal(
        &mut self,
        shape: StaticNominalShape<'a>,
        name: &str,
        binders: &[HirSignatureBinder],
        parameters: &[TypeParamDecl],
    ) {
        let kind = match shape.kind() {
            StaticNominalKind::Struct => "struct".into(),
            StaticNominalKind::Enum => "enum".into(),
            StaticNominalKind::Class => "class".into(),
            StaticNominalKind::Object => "object".into(),
            StaticNominalKind::Interface => "interface".into(),
            StaticNominalKind::Intrinsic(kind) => format!("intrinsic {}", kind.name()),
        };
        writeln!(self.out, "{kind} {name} {:?}", shape.declaration()).unwrap();
        self.annotations(shape.annotations(self.world), 1);
        for parameter in parameters {
            let bounds = match &parameter.bounds {
                TypeParamBounds::Unconstrained => "Any".to_owned(),
                TypeParamBounds::Value { .. } => "value".to_owned(),
                TypeParamBounds::Ref { .. } => "ref".to_owned(),
                TypeParamBounds::Nominal(bounds) => bounds
                    .in_source_order()
                    .iter()
                    .map(|bound| {
                        self.type_name(
                            &shape
                                .type_use(bound.ty())
                                .signature(self.module, binders)
                                .unwrap(),
                            parameters,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" & "),
            };
            writeln!(self.out, "  parameter {}: {bounds}", parameter.name).unwrap();
        }
        if let Some(base) = shape.base_class(self.world) {
            let base = self.type_name(&base.signature(self.module, binders).unwrap(), parameters);
            writeln!(self.out, "  base {base}").unwrap();
        }
        for interface in shape.interfaces(self.world) {
            let interface = self.type_name(
                &interface.signature(self.module, binders).unwrap(),
                parameters,
            );
            writeln!(self.out, "  interface {interface}").unwrap();
        }
        for field in shape.fields() {
            self.field(field, binders, parameters, 1);
        }
        for property in shape.properties(self.world) {
            self.property(property, binders, parameters);
        }
        if let Some(constructor) = shape.primary_constructor(self.world) {
            self.constructor(constructor, binders, parameters, 1);
        }
        for variant in shape.variants() {
            writeln!(
                self.out,
                "  variant {} {:?} {}",
                variant.name(),
                variant.style(),
                variant.identity()
            )
            .unwrap();
            self.annotations(variant.annotations(self.world), 2);
            for field in variant.fields() {
                self.field(field, binders, parameters, 2);
            }
            self.constructor(variant.constructor(self.world), binders, parameters, 2);
        }
    }

    fn annotations(&mut self, annotations: StaticAnnotations<'a>, indent: usize) {
        for annotation in annotations.iter() {
            writeln!(
                self.out,
                "{}@{} {} {:?}",
                "  ".repeat(indent),
                annotation.name(self.module, self.world),
                annotation.annotation,
                annotation.arguments
            )
            .unwrap();
        }
    }
}
