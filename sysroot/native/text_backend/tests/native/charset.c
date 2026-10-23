#include "scoop_text_backend.h"
#include <stddef.h>
#include <string.h>

_Static_assert(sizeof(scoop_charset_options) == 24, "charset options layout");
_Static_assert(sizeof(scoop_charset_result) == 16, "charset handle layout");
_Static_assert(sizeof(scoop_charset_step) == 40, "charset step layout");
_Static_assert(offsetof(scoop_charset_step, code) == 28, "charset error offset");
#define CHECK(condition) do { if (!(condition)) return __LINE__; } while (0)

int32_t scoop_test_charset_abi(void) {
  scoop_text_result name = scoop_charset_name((const uint8_t *)"ascii", 5);
  CHECK(!name.code && name.size == 8 && !memcmp(name.data, "US-ASCII", 8));
  scoop_text_free(name.data, name.size);
  CHECK(scoop_charset_name(NULL, 0).code == 1);
  CHECK(scoop_charset_name(NULL, -1).code == 9);
  CHECK(scoop_charset_name(NULL, 1).code == 9);
  scoop_charset_options options = {0};
  const uint8_t utf8[] = "UTF-8", ascii[] = "US-ASCII";
  CHECK(scoop_charset_new(NULL, 0, utf8, 5, options).code == 1);
  options.emit_bom = 2;
  CHECK(scoop_charset_new(utf8, 5, ascii, 8, options).code == 8);
  options.emit_bom = 0;
  scoop_charset_result engine = scoop_charset_new(utf8, 5, ascii, 8, options);
  CHECK(!engine.code && engine.handle);
  CHECK(scoop_charset_process(0, NULL, 0, NULL, 0, 0).code == 9);
  CHECK(scoop_charset_process(engine.handle, NULL, -1, NULL, 0, 0).code == 9);
  CHECK(scoop_charset_process(engine.handle, NULL, 1, NULL, 0, 0).code == 9);
  CHECK(scoop_charset_process(engine.handle, NULL, 0, NULL, 1, 0).code == 9);
  CHECK(scoop_charset_process(engine.handle, NULL, 0, NULL, 0, 2).code == 9);
  uint8_t output[8] = {0};
  const uint8_t input[] = {'A', 0, 0xc3, 0xa9};
  scoop_charset_step step = scoop_charset_process(engine.handle, input, 4, output, 8, 1);
  CHECK(step.code == 4 && step.offset == 2 && step.produced == 2);
  CHECK(step.consumed == 4 && output[0] == 'A' && !output[1]);
  scoop_charset_free(engine.handle);
  scoop_charset_free(0);
  engine = scoop_charset_new(utf8, 5, utf8, 5, options);
  CHECK(!engine.code);
  step = scoop_charset_process(engine.handle, NULL, 0, output, 8, 1);
  CHECK(!step.code && step.status == 2 && !step.produced && !step.consumed);
  step = scoop_charset_process(engine.handle, NULL, 0, NULL, 0, 1);
  CHECK(!step.code && step.status == 2);
  scoop_charset_free(engine.handle);
  return 0;
}
