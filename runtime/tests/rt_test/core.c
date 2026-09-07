#include "support.h"

void run_core_runtime_tests(void) {
    const ScoopString *a = (const ScoopString *)&hello;
    const ScoopString *b = (const ScoopString *)&world;

    /* concat: "hello" + "world" -> "helloworld" */
    const ScoopString *concat = scoop_rt_string_concat(a, b);
    scoop_rt_println(concat);

    /* structural equality: same content, different content, length
     * mismatch */
    const ScoopString *copy = scoop_rt_string_concat(a, b);
    scoop_rt_println_boolean(scoop_rt_string_eq(concat, copy));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, b));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, concat));
    if (scoop_rt_string_compare(a, b) >= 0 ||
        scoop_rt_string_compare(b, a) <= 0 ||
        scoop_rt_string_compare(a, a) != 0) {
        abort();
    }

    /* Long / Boolean output (print variants run into the println line). */
    scoop_rt_print_long(42);
    scoop_rt_println_long(-7);
    scoop_rt_print_boolean(true);
    scoop_rt_println_boolean(false);

    /* Core representation helpers used by ordinary Scoop methods. */
    scoop_rt_println(scoop_rt_long_to_string(42));
    scoop_rt_println(scoop_rt_long_to_string(-7));
    scoop_rt_println(scoop_rt_ulong_to_string(UINT64_MAX));
    scoop_rt_println(scoop_rt_bool_to_string(true));
    scoop_rt_println(scoop_rt_bool_to_string(false));
    scoop_rt_println_boolean(scoop_rt_bool_equals(true, true));
    scoop_rt_println_boolean(scoop_rt_long_hash(42) == scoop_rt_long_hash(42));
    scoop_rt_println_boolean(scoop_rt_ulong_hash(42) == scoop_rt_ulong_hash(42));
    scoop_rt_println_boolean(scoop_rt_bool_hash(true) == scoop_rt_bool_hash(true));
    scoop_rt_println_boolean(scoop_rt_string_hash(a) == scoop_rt_string_hash(a));

    /* scoop_rt_array_clone (M5/M14): independent snapshot — mutating the
     * original after the clone must not affect the copy, and the copy
     * receives the complete target nominal descriptor. The source is a stack object
     * laid out like a codegen array (16-byte header). */
    const ScoopTypeDescriptor array_td = {
        100, 8, 8, NULL, NULL, NULL, NULL, 0, "Array<Long>"};
    const ScoopTypeDescriptor mutable_array_td = {
        101, 8, 8, NULL, NULL, NULL, NULL, 0, "MutableArray<Long>"};
    struct {
        const ScoopTypeDescriptor *td;
        uint64_t gc_word;
        uint64_t size;
        int64_t data[3];
    } original = {&array_td, 0, 3, {10, 20, 30}};
    const ScoopArray *clone =
        scoop_rt_array_clone(&original, &mutable_array_td, sizeof(int64_t), 24);
    original.data[0] = 99;
    const int64_t *snapshot = (const int64_t *)clone->elements;
    scoop_rt_println_boolean(snapshot[0] == 10 && snapshot[1] == 20 && snapshot[2] == 30);
    scoop_rt_println_boolean(clone->size == 3 && clone->header.td == &mutable_array_td);

    /* scoop_rt_box (M6): header + payload copy. */
    int64_t payload[2] = {1, 2};
    const ScoopObjectHeader *boxed =
        scoop_rt_box(&point_td, payload, sizeof payload, NULL);
    scoop_rt_println_boolean(boxed->td == &point_td);
    const int64_t *boxed_payload = (const int64_t *)((const char *)boxed + sizeof(ScoopObjectHeader));
    scoop_rt_println_boolean(boxed_payload[0] == 1 && boxed_payload[1] == 2);

    /* scoop_rt_is_instance (M6): own td, parent chain, itable key;
     * unrelated td does not match. */
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &point_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &shape_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &describable_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &scoop_td_String));

    /* scoop_rt_itable_lookup (M6): returns the recorded slots array. */
    const void *const *slots = scoop_rt_itable_lookup(&point_td, &describable_td);
    scoop_rt_println_boolean(slots == point_describable_slots);
    typedef int64_t (*DescribeFn)(const void *);
    scoop_rt_println_long(((DescribeFn)slots[0])(boxed));

    /* scoop_rt_trap (M3) aborts the process, so it is not exercised
     * here; its trap path is covered end-to-end by EXPECT-TRAP compiler
     * fixtures. */
}
