#include "../native.h"
#include <assert.h>

typedef struct { int64_t value; } Word;
typedef struct { Hfa4 value; } NestedHfa;
Hfa4 m34_art_hfa(Hfa4 value) { value.d += 0.5; return value; }
Word m34_art_word(Word value) { value.value += 5; return value; }
NestedHfa m34_art_nested(NestedHfa value) { value.value.c += 0.5; return value; }
Big m34_art_big(Big value) {
    assert(value.a == 1 && value.b == 2 && value.c == 3 && value.d == 4);
    value.e += 5;
    return value;
}
