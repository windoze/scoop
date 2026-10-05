use super::*;

impl Lowerer {
    pub(super) fn dependency_decoding_wire_name(
        &self,
        target: hir::AnnotationTargetV1,
        name: &str,
    ) -> Option<String> {
        let annotations = self
            .dependencies
            .as_ref()
            .expect("a dependency target retains its catalog")
            .annotations(target);
        let transient = self.core_source_annotation("Transient");
        if annotations
            .iter()
            .any(|annotation| Some(annotation.annotation) == transient)
        {
            return None;
        }
        let serial = self.core_source_annotation("SerialName");
        annotations
            .iter()
            .find(|annotation| Some(annotation.annotation) == serial)
            .map_or_else(
                || Some(name.into()),
                |annotation| {
                    let hir::CanonicalConstValueV1::String(value) = &annotation.arguments[0] else {
                        unreachable!("SerialName has a String parameter")
                    };
                    Some(value.clone())
                },
            )
    }
}
