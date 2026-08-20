#include "scoop_rt.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>

typedef struct InterfaceValue {
    void *object;
    const void *table;
} InterfaceValue;

_Static_assert(sizeof(InterfaceValue) == 16 && _Alignof(InterfaceValue) == 8 &&
               offsetof(InterfaceValue, table) == 8, "Scoop interface storage");

InterfaceValue m34_interface(void *first, const void *first_table,
                             void *second, const void *second_table,
                             bool choose_first) {
    void **slots[] = {&first, &second};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 2);
    scoop_runtime_gc_collect();
    InterfaceValue result = choose_first
        ? (InterfaceValue){first, first_table}
        : (InterfaceValue){second, second_table};
    scoop_rt_pop_native_roots(&frame);
    return result;
}

InterfaceValue m34_optional_interface(void *object, const void *table) {
    if (object == NULL && table != NULL) abort();
    void **slots[] = {&object};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    InterfaceValue result = {object, table};
    scoop_rt_pop_native_roots(&frame);
    return result;
}

InterfaceValue m34_empty_interface(void *object, const void *table) {
    if (object == NULL || table != NULL) abort();
    return m34_optional_interface(object, table);
}
