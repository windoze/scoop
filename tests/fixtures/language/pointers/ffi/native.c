#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct NativeCell {
    int32_t value;
} NativeCell;

typedef int32_t (*UnaryCallback)(int32_t);

typedef struct PointerBundle {
    void *opaque;
    NativeCell *cell;
    void *optional_opaque;
    NativeCell *optional_cell;
    UnaryCallback callback;
    UnaryCallback optional_callback;
} PointerBundle;

_Static_assert(sizeof(void *) == sizeof(uint64_t), "64-bit data pointer");
_Static_assert(
    sizeof(UnaryCallback) == sizeof(uint64_t),
    "64-bit code pointer"
);

static int32_t m22_native_increment_100(int32_t value) {
    return value + 100;
}

static int32_t m22_fallback_opaque = 0;
static NativeCell m22_fallback_cell = {0};

void *m22_global_opaque = &m22_fallback_opaque;
NativeCell *m22_global_cell = &m22_fallback_cell;
void *m22_global_optional_opaque = NULL;
NativeCell *m22_global_optional_cell = NULL;
UnaryCallback m22_global_callback = m22_native_increment_100;
UnaryCallback m22_global_optional_callback = NULL;

static void *m22_owned_global_opaque = NULL;
static NativeCell *m22_owned_global_cell = NULL;
static void *m22_owned_global_optional_opaque = NULL;
static NativeCell *m22_owned_global_optional_cell = NULL;

static void *m22_must_allocate(size_t size) {
    void *allocation = malloc(size);
    if (allocation == NULL) {
        abort();
    }
    return allocation;
}

void *m22_round_opaque(void *pointer) {
    return pointer;
}

NativeCell *m22_round_cell(NativeCell *pointer) {
    return pointer;
}

void *m22_round_optional_opaque(void *pointer) {
    return pointer;
}

NativeCell *m22_round_optional_cell(NativeCell *pointer) {
    return pointer;
}

UnaryCallback m22_round_callback(UnaryCallback callback) {
    return callback;
}

UnaryCallback m22_round_optional_callback(UnaryCallback callback) {
    return callback;
}

void *m22_null_opaque(void) {
    return NULL;
}

NativeCell *m22_null_cell(void) {
    return NULL;
}

UnaryCallback m22_native_callback(void) {
    return m22_native_increment_100;
}

UnaryCallback m22_native_optional_callback(void) {
    return m22_native_increment_100;
}

UnaryCallback m22_null_callback(void) {
    return NULL;
}

int32_t m22_apply_callback(int32_t value, UnaryCallback callback) {
    return callback(value);
}

PointerBundle m22_round_bundle(PointerBundle bundle) {
    return bundle;
}

PointerBundle m22_make_some_bundle(void *opaque, NativeCell *cell) {
    PointerBundle bundle = {
        opaque,
        cell,
        opaque,
        cell,
        m22_native_increment_100,
        m22_native_increment_100,
    };
    return bundle;
}

PointerBundle m22_make_none_bundle(void *opaque, NativeCell *cell) {
    PointerBundle bundle = {
        opaque,
        cell,
        NULL,
        NULL,
        m22_native_increment_100,
        NULL,
    };
    return bundle;
}

int32_t m22_read_opaque(void *pointer) {
    return *(const int32_t *)pointer;
}

void m22_prepare_globals(void) {
    if (m22_owned_global_opaque != NULL) {
        abort();
    }

    m22_owned_global_opaque = m22_must_allocate(sizeof(int32_t));
    m22_owned_global_cell = m22_must_allocate(sizeof(NativeCell));
    m22_owned_global_optional_opaque = m22_must_allocate(sizeof(int32_t));
    m22_owned_global_optional_cell = m22_must_allocate(sizeof(NativeCell));

    *(int32_t *)m22_owned_global_opaque = 501;
    m22_owned_global_cell->value = 502;
    *(int32_t *)m22_owned_global_optional_opaque = 503;
    m22_owned_global_optional_cell->value = 504;

    m22_global_opaque = m22_owned_global_opaque;
    m22_global_cell = m22_owned_global_cell;
    m22_global_optional_opaque = m22_owned_global_optional_opaque;
    m22_global_optional_cell = m22_owned_global_optional_cell;
    m22_global_callback = m22_native_increment_100;
    m22_global_optional_callback = m22_native_increment_100;
}

bool m22_globals_match(
    void *opaque,
    NativeCell *cell,
    void *optional_opaque,
    NativeCell *optional_cell,
    UnaryCallback callback,
    UnaryCallback optional_callback
) {
    if (m22_global_opaque != opaque || m22_global_cell != cell) {
        return false;
    }
    if (m22_global_optional_opaque != optional_opaque ||
        m22_global_optional_cell != optional_cell) {
        return false;
    }
    if (m22_global_callback != callback ||
        m22_global_optional_callback != optional_callback) {
        return false;
    }
    if (m22_global_callback == NULL || m22_global_callback(40) != 41) {
        return false;
    }
    return m22_global_optional_callback == NULL ||
           m22_global_optional_callback(41) == 42;
}

void m22_cleanup_globals(void) {
    m22_global_opaque = &m22_fallback_opaque;
    m22_global_cell = &m22_fallback_cell;
    m22_global_optional_opaque = NULL;
    m22_global_optional_cell = NULL;
    m22_global_callback = m22_native_increment_100;
    m22_global_optional_callback = NULL;

    free(m22_owned_global_opaque);
    free(m22_owned_global_cell);
    free(m22_owned_global_optional_opaque);
    free(m22_owned_global_optional_cell);

    m22_owned_global_opaque = NULL;
    m22_owned_global_cell = NULL;
    m22_owned_global_optional_opaque = NULL;
    m22_owned_global_optional_cell = NULL;
}
