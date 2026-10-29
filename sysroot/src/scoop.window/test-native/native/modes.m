#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>
#include <stdatomic.h>
#include <stdint.h>

static _Atomic(int32_t) active_mode;
static _Atomic(int32_t) completed_modes;

@interface SWTestMode : NSObject
@property(nonatomic) int32_t kind;
@property(nonatomic, strong) NSMenu *menu;
@end
@implementation SWTestMode
@end

static void end_mode(CFRunLoopTimerRef timer, void *context) {
  SWTestMode *mode = (__bridge SWTestMode *)context;
  if (mode.kind == 1)
    [NSApp stopModal];
  else
    [mode.menu cancelTracking];
}

void scoop_window_test_schedule_mode(void *pointer, int32_t kind,
                                     double seconds) {
  NSWindow *window = (__bridge NSWindow *)pointer;
  CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{
    @autoreleasepool {
      SWTestMode *mode = [SWTestMode new];
      mode.kind = kind;
      mode.menu = [[NSMenu alloc] initWithTitle:@"Scoop native host test"];
      [mode.menu addItemWithTitle:@"Native tracking loop"
                           action:nil
                    keyEquivalent:@""];
      CFRunLoopTimerContext context = {0};
      context.info = (__bridge void *)mode;
      CFRunLoopTimerRef timer =
          CFRunLoopTimerCreate(NULL, CFAbsoluteTimeGetCurrent() + seconds, 0, 0,
                               0, end_mode, &context);
      CFRunLoopAddTimer(CFRunLoopGetMain(), timer, kCFRunLoopCommonModes);
      atomic_store(&active_mode, kind);
      if (kind == 1)
        [NSApp runModalForWindow:window];
      else
        [mode.menu popUpMenuPositioningItem:nil
                                 atLocation:NSMakePoint(12, 12)
                                     inView:window.contentView];
      CFRunLoopTimerInvalidate(timer);
      CFRelease(timer);
      atomic_store(&active_mode, 0);
      atomic_fetch_add(&completed_modes, 1);
    }
  });
  CFRunLoopWakeUp(CFRunLoopGetMain());
}

int32_t scoop_window_test_active_mode(void) {
  return atomic_load(&active_mode);
}
int32_t scoop_window_test_completed_modes(void) {
  return atomic_load(&completed_modes);
}
