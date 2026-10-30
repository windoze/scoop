#include <windows.h>

#include <stdint.h>

int scoop_fixture_imports(void) {
  // Both SDK import archives offer RtlLookupFunctionEntry. This must remain
  // an ordinary lazy PE import choice when both libraries are required.
  DWORD64 image = 0;
  RtlLookupFunctionEntry((DWORD64)(uintptr_t)&GetModuleHandleW, &image, NULL);
  return GetModuleHandleW(NULL) != NULL ? 42 : 1;
}
