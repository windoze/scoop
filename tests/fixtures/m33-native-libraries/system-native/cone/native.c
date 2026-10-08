#if SCOOP_TARGET_OS_DARWIN
#include <CoreFoundation/CoreFoundation.h>
#else
#include <math.h>
#endif
int m33_system_value(void) {
#if SCOOP_TARGET_OS_DARWIN
  CFStringRef value =
      CFStringCreateWithCString(NULL, "scoop", kCFStringEncodingUTF8);
  int result = (int)CFStringGetLength(value) + 37;
  CFRelease(value);
  return result;
#else
  volatile double argument = 0.0;
  return cos(argument) == 1.0 ? 42 : 1;
#endif
}
