#include "scoop_text_backend.h"
#include <stddef.h>
#include <string.h>

_Static_assert(sizeof(scoop_idna_result) == 32, "IDNA result layout");
_Static_assert(offsetof(scoop_idna_result, label) == 24, "IDNA label offset");
_Static_assert(sizeof(scoop_idna_name) == 48, "IDNA name layout");
_Static_assert(offsetof(scoop_idna_name, label) == 40, "IDNA name label offset");
_Static_assert(sizeof(scoop_text_result) == 24, "NFC result layout");

#define CHECK(condition) do { if (!(condition)) return __LINE__; } while (0)

int32_t scoop_test_text_abi(void) {
  scoop_idna_result empty = scoop_idna_punycode(NULL, 0, false);
  CHECK(!empty.code && !empty.data && !empty.size);
  scoop_text_free(empty.data, empty.size);
  CHECK(scoop_idna_domain(NULL, 0, 0).code != 0);
  CHECK(scoop_idna_domain(NULL, 1, 0).code == -1007);
  CHECK(scoop_idna_domain(NULL, -1, 0).code == -1007);
  CHECK(scoop_idna_domain(NULL, 0, 4).code == -1007);
  CHECK(scoop_charset_normalize_nfc(NULL, 1).code == 9);
  const uint8_t malformed[] = {0xff};
  CHECK(scoop_idna_punycode(malformed, 1, false).code == -1007);

  const uint8_t domain[] = "B\xc3\x9c" "CHER.example.";
  scoop_idna_name name = scoop_idna_parse(domain, sizeof(domain) - 1);
  CHECK(!name.code && name.ascii_size == 22 && name.unicode_size == 16);
  CHECK(!memcmp(name.ascii, "xn--bcher-kva.example.", (size_t)name.ascii_size));
  CHECK(!memcmp(name.unicode, "b\xc3\xbc" "cher.example.", (size_t)name.unicode_size));
  scoop_text_free(name.ascii, name.ascii_size);
  scoop_text_free(name.unicode, name.unicode_size);

  const uint8_t raw[] = {'a', 0, '.', 0xc3, 0xa9};
  scoop_idna_result encoded = scoop_idna_punycode(raw, sizeof(raw), false);
  CHECK(!encoded.code);
  scoop_idna_result decoded = scoop_idna_punycode(encoded.data, encoded.size, true);
  scoop_text_free(encoded.data, encoded.size);
  CHECK(!decoded.code && decoded.size == sizeof(raw));
  CHECK(!memcmp(decoded.data, raw, sizeof(raw)));
  scoop_text_free(decoded.data, decoded.size);

  const uint8_t decomposed[] = {0, 'e', 0xcc, 0x81};
  const uint8_t composed[] = {0, 0xc3, 0xa9};
  scoop_text_result normalized = scoop_charset_normalize_nfc(decomposed, sizeof(decomposed));
  CHECK(!normalized.code && normalized.size == sizeof(composed));
  CHECK(!memcmp(normalized.data, composed, sizeof(composed)));
  scoop_text_free(normalized.data, normalized.size);
  normalized = scoop_charset_normalize_nfc(NULL, 0);
  CHECK(!normalized.code && !normalized.size && !normalized.data);
  scoop_text_free(normalized.data, normalized.size);
  return 0;
}
