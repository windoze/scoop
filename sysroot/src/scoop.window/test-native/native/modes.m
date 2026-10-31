#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>
#include <stdatomic.h>
#include <stdint.h>

static _Atomic(int32_t) active_mode;
static _Atomic(int32_t) completed_modes;

@interface SWTestMode : NSObject
@property(nonatomic) int32_t kind;
@property(nonatomic) double seconds;
@property(nonatomic) CFRunLoopTimerRef timer;
@property(nonatomic) BOOL entered;
@property(nonatomic, strong) NSMenu *menu;
@end
@implementation SWTestMode
@end

static void enter_mode(CFRunLoopObserverRef observer,
                       CFRunLoopActivity activity, void *context) {
  SWTestMode *mode = (__bridge SWTestMode *)context;
  if (mode.entered)
    return;
  CFStringRef current = CFRunLoopCopyCurrentMode(CFRunLoopGetMain());
  NSString *expected =
      mode.kind == 1 ? NSModalPanelRunLoopMode : NSEventTrackingRunLoopMode;
  BOOL matches = current && CFEqual(current, (__bridge CFStringRef)expected);
  if (current)
    CFRelease(current);
  if (matches) {
    mode.entered = YES;
    atomic_store(&active_mode, mode.kind);
    CFRunLoopTimerSetNextFireDate(mode.timer,
                                  CFAbsoluteTimeGetCurrent() + mode.seconds);
  }
}

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
      mode.seconds = seconds;
      mode.menu = [[NSMenu alloc] initWithTitle:@"Scoop native host test"];
      [mode.menu addItemWithTitle:@"Native tracking loop"
                           action:nil
                    keyEquivalent:@""];
      CFRunLoopTimerContext context = {0};
      context.info = (__bridge void *)mode;
      CFRunLoopTimerRef timer = CFRunLoopTimerCreate(
          NULL, CFAbsoluteTimeGetCurrent() + 5.0, 0, 0, 0, end_mode, &context);
      mode.timer = timer;
      CFRunLoopObserverContext observer_context = {0};
      observer_context.info = (__bridge void *)mode;
      CFRunLoopObserverRef observer = CFRunLoopObserverCreate(
          NULL, kCFRunLoopEntry, true, 0, enter_mode, &observer_context);
      CFRunLoopAddObserver(CFRunLoopGetMain(), observer, kCFRunLoopCommonModes);
      CFRunLoopAddTimer(CFRunLoopGetMain(), timer, kCFRunLoopCommonModes);
      if (kind == 1)
        [NSApp runModalForWindow:window];
      else
        [mode.menu popUpMenuPositioningItem:nil
                                 atLocation:NSMakePoint(12, 12)
                                     inView:window.contentView];
      CFRunLoopTimerInvalidate(timer);
      CFRelease(timer);
      CFRunLoopObserverInvalidate(observer);
      CFRelease(observer);
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
