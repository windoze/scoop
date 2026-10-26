#include <CoreFoundation/CoreFoundation.h>
#include <dispatch/dispatch.h>

void scoop_test_main_turn(void) { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true); }
