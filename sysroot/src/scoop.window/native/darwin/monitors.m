#import "window.h"
#import <CoreGraphics/CoreGraphics.h>
#include <math.h>

@interface SWMonitorRecord : NSObject
@property(nonatomic) SWMonitorData data;
@property(nonatomic, strong) NSData *name;
@end
@implementation SWMonitorRecord
@end

void *scoop_window_monitors(void *application) {
  @autoreleasepool {
    NSArray<NSScreen *> *screens = NSScreen.screens;
    NSMutableArray<SWMonitorRecord *> *result = [NSMutableArray new];
    double top = NSMaxY(screens.firstObject.frame);
    for (NSScreen *screen in screens) {
      SWMonitorRecord *record = [SWMonitorRecord new];
      SWMonitorData data = {0};
      CGDirectDisplayID display =
          [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue];
      data.position_known = 1;
      data.key = display;
      data.primary = screen == screens.firstObject;
      NSRect frame = screen.frame;
      NSRect pixels = [screen convertRectToBacking:frame];
      data.x = frame.origin.x;
      data.y = top - NSMaxY(frame);
      data.width = frame.size.width;
      data.height = frame.size.height;
      data.pixel_width = ceil(pixels.size.width);
      data.pixel_height = ceil(pixels.size.height);
      data.scale = screen.backingScaleFactor;
      CGDisplayModeRef mode = CGDisplayCopyDisplayMode(display);
      if (mode) {
        double millihertz = CGDisplayModeGetRefreshRate(mode) * 1000.0;
        if (isfinite(millihertz) && millihertz > 0 &&
            millihertz < (double)INT64_MAX)
          data.refresh_millihertz = llround(millihertz);
        CGDisplayModeRelease(mode);
      }
      record.data = data;
      record.name =
          [screen.localizedName dataUsingEncoding:NSUTF8StringEncoding];
      [result addObject:record];
    }
    return (__bridge_retained void *)[result copy];
  }
}

int64_t scoop_window_monitors_count(void *pointer) {
  return (int64_t)((__bridge NSArray *)pointer).count;
}

SWMonitorData scoop_window_monitor_data(void *pointer, int64_t index) {
  SWMonitorRecord *record = ((__bridge NSArray *)pointer)[(NSUInteger)index];
  SWMonitorData data = record.data;
  data.name = record.name.bytes;
  data.name_length = (int64_t)record.name.length;
  return data;
}

void scoop_window_monitors_release(void *pointer) {
  (void)CFBridgingRelease(pointer);
}

int64_t scoop_window_current_monitor(void *pointer) {
  NSScreen *screen = ((__bridge SWWindow *)pointer).window.screen;
  return [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue];
}
