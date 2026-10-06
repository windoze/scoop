use super::*;

impl Lowerer {
    pub(in crate::class::inheritance) fn dependency_coding_wire_name(
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
    pub(in crate::class) fn serialization_wire_name(
        &self,
        target: hir::SourceAnnotationTarget,
        source_name: &str,
    ) -> Option<String> {
        let applications = self
            .annotation_metadata
            .targets
            .iter()
            .find(|value| value.target == target)
            .map_or(&[][..], |value| value.annotations.as_slice());
        let transient = self.core_source_annotation("Transient");
        if applications
            .iter()
            .any(|value| Some(value.annotation) == transient)
        {
            return None;
        }
        let serial_name = self.core_source_annotation("SerialName");
        applications
            .iter()
            .find(|value| Some(value.annotation) == serial_name)
            .map_or_else(
                || Some(source_name.to_owned()),
                |value| match &value.arguments[0] {
                    hir::CanonicalConstValueV1::String(name) => Some(name.clone()),
                    _ => unreachable!("SerialName has a checked String argument"),
                },
            )
    }
}
