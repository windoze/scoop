//! Emit the selected image tables through the existing startup C producer.

use crate::program::image::SelectedImage;
use std::fmt::Write;

pub(super) const TYPES: &str = r#"
typedef struct { const uint8_t *data; uint64_t length; } ScoopByteSpanV1;
typedef struct { const void *const *records; uint64_t count; } ScoopImageTableV1;
typedef struct ScoopImageDescriptorV1 {
    uint64_t magic;
    uint32_t abi_version, struct_size;
    ScoopByteSpanV1 coordinate[3];
    uint8_t cone[32], runtime_image_fingerprint[32];
    const uint8_t (*dependencies)[32];
    uint64_t dependency_count;
    ScoopImageTableV1 tables[6];
} ScoopImageDescriptorV1;
_Static_assert(sizeof(ScoopImageDescriptorV1) == 240, "image ABI size");
_Static_assert(offsetof(ScoopImageDescriptorV1, runtime_image_fingerprint) == 96, "image digest offset");
_Static_assert(offsetof(ScoopImageDescriptorV1, tables) == 144, "image tables offset");
typedef struct ScoopRootEntryDescriptorV1 ScoopRootEntryDescriptorV1;
"#;

pub(super) fn emit(source: &mut String, images: &[SelectedImage], section: &str) {
    for (index, image) in images.iter().enumerate() {
        for (part, bytes) in image.coordinate.iter().enumerate() {
            writeln!(
                source,
                "static const uint8_t coordinate_{index}_{part}[] = {{{}}};",
                initializer(bytes)
            )
            .unwrap();
        }
        writeln!(
            source,
            "static const uint8_t dependencies_{index}[][32] = {{"
        )
        .unwrap();
        if image.dependencies.is_empty() {
            source.push_str("    {0},\n");
        }
        for dependency in &image.dependencies {
            writeln!(source, "    {{{}}},", initializer(dependency)).unwrap();
        }
        source.push_str("};\n");
        for (table, records) in image.tables.iter().enumerate() {
            for (record, name) in records.iter().enumerate() {
                writeln!(
                    source,
                    "extern const uint8_t record_{index}_{table}_{record}[] __asm__(\"{name}\");"
                )
                .unwrap();
            }
            writeln!(source, "__attribute__((used, section(\"{section}\")))\nstatic const void *const table_{index}_{table}[] = {{").unwrap();
            if records.is_empty() {
                source.push_str("    0,\n");
            }
            for record in 0..records.len() {
                writeln!(source, "    record_{index}_{table}_{record},").unwrap();
            }
            source.push_str("};\n");
        }
        for (trap, (name, bytes)) in image.traps.iter().enumerate() {
            writeln!(source, "const uint8_t trap_{index}_{trap}[] __asm__(\"{name}\") __attribute__((used, visibility(\"hidden\"))) = {{{}}};", initializer(bytes)).unwrap();
        }
        writeln!(source, "const ScoopImageDescriptorV1 image_{index} __asm__(\"{}\") __attribute__((used, visibility(\"hidden\"), section(\"{section}\"))) = {{", image.name).unwrap();
        source.push_str("    UINT64_C(0x53434f4f50494d47), 6, 240,\n    {\n");
        for (part, bytes) in image.coordinate.iter().enumerate() {
            writeln!(
                source,
                "        {{coordinate_{index}_{part}, {}}},",
                bytes.len()
            )
            .unwrap();
        }
        writeln!(
            source,
            "    }},\n    {{{}}},\n    {{{}}},\n    dependencies_{index}, {},\n    {{",
            initializer(&image.cone),
            initializer(&image.fingerprint),
            image.dependencies.len()
        )
        .unwrap();
        for (table, records) in image.tables.iter().enumerate() {
            writeln!(
                source,
                "        {{table_{index}_{table}, {}}},",
                records.len()
            )
            .unwrap();
        }
        source.push_str("    },\n};\n");
    }
}

fn initializer(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0".into();
    }
    bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ")
}
