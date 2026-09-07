#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef void (*ScoopTestFunctionPointer)(void);

_Static_assert(CHAR_BIT == 8, "Scoop requires 8-bit bytes");
_Static_assert(UINTPTR_MAX == UINT64_MAX,
               "uintptr_t must expose every 64-bit carrier value");
_Static_assert(sizeof(uintptr_t) == sizeof(uint64_t),
               "uintptr_t must preserve every qualified data address");
_Static_assert(sizeof(void *) == sizeof(uint64_t),
               "data pointers must fit the qualified carrier");
_Static_assert(_Alignof(void *) == _Alignof(uint64_t),
               "data pointer alignment must match the qualified carrier");
_Static_assert(sizeof(ScoopTestFunctionPointer) == sizeof(uint64_t),
               "function pointers must fit the qualified carrier");
_Static_assert(_Alignof(ScoopTestFunctionPointer) == _Alignof(uint64_t),
               "function pointer alignment must match the qualified carrier");

static void qualified_function_address(void) {}

static int data_pointer_representation_is_qualified(void) {
    void *null_pointer = NULL;
    uint64_t null_bits = UINT64_MAX;
    memcpy(&null_bits, &null_pointer, sizeof(null_bits));
    if (null_bits != 0) {
        return 0;
    }

    uint64_t object = UINT64_C(0x123456789abcdef0);
    void *address = &object;
    uint64_t address_bits = 0;
    memcpy(&address_bits, &address, sizeof(address_bits));
    if (address_bits == 0) {
        return 0;
    }
    void *round_trip = NULL;
    memcpy(&round_trip, &address_bits, sizeof(round_trip));
    return round_trip == address;
}

static int function_pointer_representation_is_qualified(void) {
    ScoopTestFunctionPointer null_pointer = NULL;
    uint64_t null_bits = UINT64_MAX;
    memcpy(&null_bits, &null_pointer, sizeof(null_bits));
    if (null_bits != 0) {
        return 0;
    }

    ScoopTestFunctionPointer address = qualified_function_address;
    uint64_t address_bits = 0;
    memcpy(&address_bits, &address, sizeof(address_bits));
    if (address_bits == 0) {
        return 0;
    }
    ScoopTestFunctionPointer round_trip = NULL;
    memcpy(&round_trip, &address_bits, sizeof(round_trip));
    return round_trip == address;
}

int main(void) {
    if (!data_pointer_representation_is_qualified() ||
        !function_pointer_representation_is_qualified()) {
        return 1;
    }
    puts("target pointer qualification tests passed");
    return 0;
}
