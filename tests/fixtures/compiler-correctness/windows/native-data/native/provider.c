#include "data.h"

EXPORT int32_t scoop_fixture_counter = 11;
EXPORT Pair scoop_fixture_pair = {13, 17};

EXPORT int32_t *scoop_fixture_counter_address(void) {
  return &scoop_fixture_counter;
}
EXPORT Pair *scoop_fixture_pair_address(void) { return &scoop_fixture_pair; }

EXPORT int32_t scoop_fixture_provider_kind(void) {
#ifdef SCOOP_DATA_DLL
  return 79;
#else
  return 53;
#endif
}
