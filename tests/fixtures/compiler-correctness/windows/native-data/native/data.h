#include <stdint.h>

#ifdef SCOOP_DATA_DLL
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

typedef struct {
  int32_t left, right;
} Pair;

EXPORT int32_t *scoop_fixture_counter_address(void);
EXPORT Pair *scoop_fixture_pair_address(void);
EXPORT int32_t scoop_fixture_provider_kind(void);
