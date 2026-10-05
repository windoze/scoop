#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/platform/platform.h"

static void check(const void *input, size_t size, const char *expected) {
    uint8_t digest[32];
    assert(scoop_platform_sha256(input, size, digest));
    char hex[65];
    for (size_t i = 0; i < sizeof digest; ++i) {
        snprintf(hex + 2 * i, 3, "%02x", digest[i]);
    }
    assert(strcmp(hex, expected) == 0);
}

int main(void) {
    check("", 0,
          "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    check("abc", 3,
          "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    const char *blocks =
        "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    check(blocks, strlen(blocks),
          "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    char *large = malloc(1000000);
    assert(large != NULL);
    memset(large, 'a', 1000000);
    check(large, 1000000,
          "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    free(large);
    puts("SHA-256 empty, short, multi-block and million-byte vectors passed");
}
