#include "support.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen.
 * Non-static: rt.c references it from scoop_rt_string_concat. */
const ScoopTypeDescriptor scoop_td_String = {
    .type_id = 1,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = sizeof(ScoopString),
            .instance_alignment = _Alignof(ScoopString),
            .inline_offset = sizeof(ScoopString),
            .inline_size = 1,
            .inline_stride = 1,
            .inline_alignment = 1,
        },
    .diagnostic_name = {(const uint8_t *)"String", sizeof("String") - 1},
};

/* Same layout codegen uses for StringConst globals:
 * { td, gc_word, len, data } (16-byte header, runtime spec 2.4). */

const FiveCharConst hello = {&scoop_td_String, 0, 5, {'h', 'e', 'l', 'l', 'o'}};
const FiveCharConst world = {&scoop_td_String, 0, 5, {'w', 'o', 'r', 'l', 'd'}};

/* M6 dispatch fixtures: interface Describable; class Shape; class
 * Point : Shape, Describable (vtable contains its ordinary describe method). Sizes
 * include the 16-byte header. */
static int64_t point_describe(const void *self) {
    (void)self;
    return 7;
}

const ScoopTypeDescriptor describable_td = {
    .type_id = 1002,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
        },
    .diagnostic_name = {(const uint8_t *)"Describable", sizeof("Describable") - 1},
};
const void *const point_describable_slots[] = {(const void *)&point_describe};
static const ScoopItableEntryV1 point_itables[] = {
    {&describable_td, point_describable_slots}};
static const void *const point_vtable[] = {(const void *)&point_describe};
const ScoopTypeDescriptor shape_td = {
    .type_id = 1000,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
            .minimum_size = 24,
            .instance_alignment = 8,
        },
    .diagnostic_name = {(const uint8_t *)"Shape", sizeof("Shape") - 1},
};
const ScoopTypeDescriptor point_td = {
    .type_id = 1001,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = 32,
            .instance_alignment = 8,
            .inline_offset = 16,
            .inline_size = 16,
            .inline_alignment = 8,
        },
    .parent = &shape_td,
    .vtable = point_vtable,
    .itables = point_itables,
    .itable_count = 1,
    .diagnostic_name = {(const uint8_t *)"Point", sizeof("Point") - 1},
};

/* M9 GC fixtures. */

/* class Node { value: i64, next: Node? } — plain layout, one reference
 * at object offset 24. */

static const uint64_t node_refs[] = {1, 24};
const ScoopTypeDescriptor node_td = {
    .type_id = 2000,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
            .minimum_size = 32,
            .instance_alignment = 8,
        },
    .object_scan = node_refs,
    .diagnostic_name = {(const uint8_t *)"Node", sizeof("Node") - 1},
};

/* Exact image metadata normally emitted by codegen. The managed global uses
 * an inline-value scan rooted at its writable pointer slot; String literals
 * are immutable, GC-free-payload managed objects with stable addresses. */
ScoopNode *image_global_rooted;
void *image_immortal_rooted;
static const uint64_t image_global_scan[] = {1, 0};
const ScoopManagedGlobalDescriptor scoop_image_managed_globals[] = {
    {&image_global_rooted, image_global_scan}};
const uint64_t scoop_image_managed_global_count = 1;
const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[] = {
    {&hello, sizeof hello, &scoop_td_String},
    {&world, sizeof world, &scoop_td_String},
};
const uint64_t scoop_image_immortal_object_count = 2;
const ScoopInitializationUnitDescriptor scoop_image_initialization_units[] = {{0}};
const uint64_t scoop_image_initialization_unit_count = 0;

/* 64-byte plain object without references: exactly two per line, for
 * the free-line reuse test. */

const ScoopTypeDescriptor big64_td = {
    .type_id = 2001,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
            .minimum_size = 64,
            .instance_alignment = 8,
        },
    .diagnostic_name = {(const uint8_t *)"Big64", sizeof("Big64") - 1},
};

/* Array with reference elements (recursive SCOOP_REFS_ARRAY scan);
 * `size` in the TD is the element stride (pointer). */
static const uint64_t ref_element_scan[] = {1, 0};
static const uint64_t ref_array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, 8,
                                          (uint64_t)(uintptr_t)ref_element_scan};
static const ScoopTypeDescriptor ref_array_td = {
    .type_id = 2100,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = 24,
            .instance_alignment = 8,
            .inline_offset = 24,
            .inline_size = 8,
            .inline_stride = 8,
            .inline_alignment = 8,
            .inline_scan = ref_element_scan,
        },
    .object_scan = ref_array_scan,
    .diagnostic_name = {(const uint8_t *)"Array<String>", sizeof("Array<String>") - 1},
};

/* Boxed tagged enum E { A(String), B(i64) }: B uses the shared pure
 * slot; A has its own ref-bearing slot. */

static const uint64_t enum_scan[] = {1, 32};
static const uint64_t enum_inline_scan[] = {1, 16};
static const ScoopTypeDescriptor enum_td = {
    .type_id = 2002,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = 40,
            .instance_alignment = 8,
            .inline_offset = 16,
            .inline_size = 24,
            .inline_alignment = 8,
            .inline_scan = enum_inline_scan,
        },
    .object_scan = enum_scan,
    .diagnostic_name = {(const uint8_t *)"E", sizeof("E") - 1},
};

/* Array<Nested>, where each inline element is
 * { tagged enum E, tail: String }. The sequence combines the
 * unconditional tail reference with a tag-selected payload scan;
 * the array wrapper repeats that recursive element scan by stride. */

static const uint64_t nested_element_scan[] = {2, 16, 24};
static const uint64_t nested_array_scan[] = {SCOOP_REFS_ARRAY, 16, 24,
                                             sizeof(ScoopNestedElement),
                                             (uint64_t)(uintptr_t)nested_element_scan};
static const ScoopTypeDescriptor nested_array_td = {
    .type_id = 2101,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = 24,
            .instance_alignment = _Alignof(ScoopNestedElement),
            .inline_offset = 24,
            .inline_size = sizeof(ScoopNestedElement),
            .inline_stride = sizeof(ScoopNestedElement),
            .inline_alignment = _Alignof(ScoopNestedElement),
            .inline_scan = nested_element_scan,
        },
    .object_scan = nested_array_scan,
    .diagnostic_name = {(const uint8_t *)"Array<Nested>", sizeof("Array<Nested>") - 1},
};

ScoopNode *new_node(int64_t value, ScoopNode *next) {
    ScoopNode *node = scoop_rt_alloc(&node_td, sizeof(ScoopNode));
    node->value = value;
    node->next = next;
    return node;
}

/* Garbage is created inside helpers so the only references live in
 * the helper's (dead) frame after it returns. */
void make_garbage_nodes(int count) {
    for (int i = 0; i < count; i++) {
        ScoopNode *junk = new_node(i, NULL);
        (void)junk;
    }
}

void make_garbage_big64(void) {
    for (int i = 0; i < 4; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

/* Enough 64B garbage to overrun one 32KB block and spill into the
 * next (600 * 64B > 255 usable lines). */
void make_garbage_big64_many(void) {
    for (int i = 0; i < 600; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

ScoopArray *make_ref_array(void) {
    ScoopArray *array =
        scoop_rt_alloc(&ref_array_td, sizeof(ScoopArray) + 2 * sizeof(uint64_t));
    array->size = 2;
    const ScoopString **elements = (const ScoopString **)array->elements;
    elements[0] = scoop_rt_long_to_string(1001);
    elements[1] = scoop_rt_long_to_string(1002);
    return array;
}

ScoopBoxedEnum *make_boxed_enum_a(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 0;
    e->a_ref = scoop_rt_long_to_string(1003);
    return e;
}

ScoopBoxedEnum *make_boxed_enum_b(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 1;
    e->pure_payload = 0xDEADBEEF0; /* pure-value payload is not scanned */
    return e;
}

ScoopArray *make_nested_array(void) {
    ScoopArray *array = scoop_rt_alloc(
        &nested_array_td, sizeof(ScoopArray) + 2 * sizeof(ScoopNestedElement));
    array->size = 2;
    ScoopNestedElement *elements = (ScoopNestedElement *)array->elements;
    elements[0].tag = 0;
    elements[0].a_ref = scoop_rt_long_to_string(1004);
    elements[0].tail = scoop_rt_long_to_string(1005);
    elements[1].tag = 1;
    elements[1].pure_payload = UINT64_C(0xDEADBEEF0);
    elements[1].tail = scoop_rt_long_to_string(1006);
    return array;
}

/* Overwrite the dead stack region below the current frame so the
 * conservative stack scan no longer finds stale object pointers left
 * behind by helpers that have returned (v1 transition-era helper; see
 * the file header comment). */
void clobber_stack(void) {
    volatile uint64_t buf[2048];
    for (size_t i = 0; i < 2048; i++) {
        buf[i] = 0;
    }
}

/* Static slot registered as a global root (not on the stack, so only
 * the root registration keeps its value alive). */
ScoopNode *global_rooted;
