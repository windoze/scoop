use super::*;
use crate::user_annotations::SerializeConfig;

impl Lowerer {
    pub(in crate::class::inheritance) fn signature_is_core_option(
        &self,
        ty: &scoop_identity::SignatureTypeKey,
    ) -> bool {
        matches!(ty, scoop_identity::SignatureTypeKey::NominalApplication { origin, .. }
            if Some(hir::SourceNominalId::GenericTemplate(*origin)) == self.core_coding_nominal("Option"))
    }

    pub(in crate::class::inheritance) fn dependency_serialization_config(
        &self,
        target: hir::AnnotationTargetV1,
    ) -> SerializeConfig {
        let annotations = self
            .dependencies
            .as_ref()
            .expect("a dependency target retains its catalog")
            .annotations(target);
        let serialize = self.core_source_annotation("Serialize");
        annotations
            .iter()
            .find(|annotation| Some(annotation.annotation) == serialize)
            .map_or_else(SerializeConfig::default, |annotation| {
                SerializeConfig::from_arguments(&annotation.arguments)
            })
    }

    pub(in crate::class) fn serialization_config(
        &self,
        target: hir::SourceAnnotationTarget,
    ) -> SerializeConfig {
        let applications = self
            .annotation_metadata
            .targets
            .iter()
            .find(|value| value.target == target)
            .map_or(&[][..], |value| value.annotations.as_slice());
        let serialize = self.core_source_annotation("Serialize");
        applications
            .iter()
            .find(|value| Some(value.annotation) == serialize)
            .map_or_else(SerializeConfig::default, |value| {
                SerializeConfig::from_arguments(&value.arguments)
            })
    }

    pub(in crate::class::inheritance) fn dependency_coding_wire_name(
        &self,
        target: hir::AnnotationTargetV1,
        name: &str,
    ) -> Option<String> {
        self.dependency_serialization_config(target).wire_name(name)
    }

    pub(in crate::class) fn serialization_wire_name(
        &self,
        target: hir::SourceAnnotationTarget,
        name: &str,
    ) -> Option<String> {
        self.serialization_config(target).wire_name(name)
    }
}
