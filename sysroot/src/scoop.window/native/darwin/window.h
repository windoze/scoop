#ifndef SCOOP_WINDOW_DARWIN_H
#define SCOOP_WINDOW_DARWIN_H

#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>
#include <stdint.h>

typedef int32_t (*SWCallback)(void *, int64_t, int32_t);
typedef struct {
  int64_t window;
  int32_t kind, flags;
  int64_t a, b, c, d;
  double x, y, z, w, scale;
  const uint8_t *text;
  int64_t text_length;
} SWEventData;
typedef struct {
  double width, height, pixel_width, pixel_height, scale;
  double x, y, outer_width, outer_height;
  int32_t visible, occluded, focused, minimized, maximized, dark;
} SWSnapshot;

@class SWApplication, SWWindow;

@interface SWEvent : NSObject
@property(nonatomic) SWEventData data;
@property(nonatomic, strong) NSData *text;
@end

@interface SWApplication : NSObject <NSApplicationDelegate>
@property(nonatomic) SWCallback callback;
@property(nonatomic) void *context;
@property(nonatomic) CFRunLoopSourceRef source;
@property(nonatomic) CFRunLoopTimerRef timer;
@property(nonatomic) BOOL deadlineFired;
@property(nonatomic, strong) NSMutableArray<SWEvent *> *events;
@property(nonatomic, strong)
    NSMutableDictionary<NSNumber *, SWWindow *> *windows;
- (void)enqueue:(SWEvent *)event;
@end

@interface SWView : NSView
@property(nonatomic, weak) SWWindow *owner;
@end

@interface SWWindow : NSObject <NSWindowDelegate>
@property(nonatomic, weak) SWApplication *application;
@property(nonatomic) int64_t identity;
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) SWView *view;
- (void)emit:(int32_t)kind;
@end

void sw_wake(SWApplication *application);
SWEvent *sw_event(SWWindow *window, int32_t kind);
SWSnapshot sw_snapshot(SWWindow *window);
double sw_desktop_top(void);

#endif
