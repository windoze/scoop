#include <stdint.h>

#include <scoop_runtime.h>

int64_t runtime_header_probe(void) {
  scoop_gc_thread_attach_current();

  if (sizeof(ScoopTypeDescriptor) == 0 || sizeof(ScoopCompositeTransportDescriptor) == 0) {
    return -1;
  }
  if (SCOOP_ARRAY_ELEM_KIND_WORD != 1u) {
    return -2;
  }

  void *arr = scoop_mutable_array_new(
      SCOOP_ARRAY_ELEM_KIND_WORD,
      (uint64_t)sizeof(uint64_t),
      (uint64_t)_Alignof(uint64_t),
      0,
      1);
  if (arr == 0) {
    return -3;
  }

  scoop_mutable_array_push_word(arr, 42u);
  if (scoop_mutable_array_to_array_data(arr) == 0) {
    return -4;
  }

  uint64_t handle = scoop_handle_new(arr);
  if (handle == 0) {
    return -5;
  }
  if (scoop_handle_get(handle) != arr) {
    return -6;
  }
  if (scoop_handle_drop(handle) != 1u) {
    return -7;
  }

  void (*detach_current)(void) = scoop_gc_thread_detach_current;
  if (detach_current == 0) {
    return -8;
  }
  return 42;
}
