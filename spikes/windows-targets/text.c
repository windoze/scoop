#include "scoop_text_backend.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

extern int seh_probe(void);

int main(void) {
    static const uint8_t decomposed[] = {'e', 0xcc, 0x81};
    scoop_text_result nfc =
        scoop_charset_normalize_nfc(decomposed, sizeof decomposed);
    assert(nfc.code == 0 && nfc.size == 2);
    assert(nfc.data[0] == 0xc3 && nfc.data[1] == 0xa9);
    scoop_text_free(nfc.data, nfc.size);

    const uint8_t domain[] = "b\xc3\xbc"
                             "cher.example";
    scoop_idna_name name = scoop_idna_parse(domain, sizeof domain - 1);
    assert(name.code == 0);
    assert(name.ascii_size == 21);
    assert(memcmp(name.ascii, "xn--bcher-kva.example", 21) == 0);
    scoop_text_free(name.ascii, name.ascii_size);
    scoop_text_free(name.unicode, name.unicode_size);

    const uint8_t utf8[] = "utf-8";
    scoop_charset_result converter =
        scoop_charset_new(utf8, 5, utf8, 5, (scoop_charset_options){0});
    assert(converter.code == 0 && converter.handle != 0);
    uint8_t output[16];
    scoop_charset_step converted =
        scoop_charset_process(converter.handle, decomposed, sizeof decomposed,
                              output, sizeof output, 1);
    assert(converted.code == 0 && converted.consumed == 3 &&
           converted.produced == 3);
    assert(memcmp(output, decomposed, 3) == 0);
    scoop_charset_free(converter.handle);
    assert(seh_probe() == 0);
    puts("Rust text allocation, native dependencies and Scoop SEH passed");
    return 0;
}
