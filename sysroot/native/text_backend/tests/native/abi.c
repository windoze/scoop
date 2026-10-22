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

  const uint8_t domain[] = "EXAMPLE.COM.";
  scoop_idna_name name = scoop_idna_parse(domain, sizeof(domain) - 1);
  CHECK(!name.code && name.ascii_size == 12 && name.unicode_size == 12);
  CHECK(!memcmp(name.ascii, "example.com.", (size_t)name.ascii_size));
  CHECK(!memcmp(name.unicode, "example.com.", (size_t)name.unicode_size));
  scoop_text_free(name.ascii, name.ascii_size);
  scoop_text_free(name.unicode, name.unicode_size);

  const uint8_t raw[] = {'a', 0, '.', 'b'};
  scoop_idna_result encoded = scoop_idna_punycode(raw, sizeof(raw), false);
  CHECK(!encoded.code);
  scoop_idna_result decoded = scoop_idna_punycode(encoded.data, encoded.size, true);
  scoop_text_free(encoded.data, encoded.size);
  CHECK(!decoded.code && decoded.size == sizeof(raw));
  CHECK(!memcmp(decoded.data, raw, sizeof(raw)));
  scoop_text_free(decoded.data, decoded.size);

  scoop_text_result normalized = scoop_charset_normalize_nfc(raw, sizeof(raw));
  CHECK(!normalized.code && normalized.size == sizeof(raw));
  CHECK(!memcmp(normalized.data, raw, sizeof(raw)));
  scoop_text_free(normalized.data, normalized.size);
  normalized = scoop_charset_normalize_nfc(NULL, 0);
  CHECK(!normalized.code && !normalized.size && !normalized.data);
  scoop_text_free(normalized.data, normalized.size);
  return 0;
}

int32_t scoop_test_unicode_abi(void) {
  CHECK(scoop_charset_grapheme_breaks(NULL, 0, NULL) == 0);
  CHECK(scoop_charset_grapheme_breaks(NULL, 1, NULL) == 9);
  CHECK(scoop_charset_grapheme_breaks(NULL, -1, NULL) == 9);
  const uint8_t cluster[] = {'e', 0xcc, 0x81};
  CHECK(scoop_charset_grapheme_breaks(cluster, sizeof(cluster), NULL) == 9);
  uint8_t breaks[] = {7, 7, 7, 7, 7};
  CHECK(!scoop_charset_grapheme_breaks(cluster, sizeof(cluster), breaks + 1));
  CHECK(breaks[0] == 7 && breaks[1] == 1 && !breaks[2] && !breaks[3] && breaks[4] == 7);
  const uint8_t malformed[] = {0xff};
  CHECK(scoop_charset_grapheme_breaks(malformed, 1, breaks) == 9);
  CHECK(scoop_charset_case(malformed, 1, 0).code == 9);
  CHECK(scoop_charset_case(NULL, 1, 0).code == 9);
  CHECK(scoop_charset_case(NULL, 0, 3).code == 9);
  CHECK(scoop_charset_scalar_width(0x110000) == -1);
  CHECK(!scoop_charset_alphanumeric(0xd800));
  for (int32_t mode = 0; mode < 3; mode++) {
    scoop_text_result empty = scoop_charset_case(NULL, 0, mode);
    CHECK(!empty.code && !empty.data && !empty.size);
    scoop_text_free(empty.data, empty.size);
  }
  const uint8_t mixed[] = {0, 'a', 'b'};
  scoop_text_result upper = scoop_charset_case(mixed, sizeof(mixed), 1);
  CHECK(!upper.code && upper.size == 3 && !memcmp(upper.data, "\0AB", 3));
  scoop_text_free(upper.data, upper.size);
  return 0;
}
