#import "window.h"
#include <math.h>

double sw_desktop_top(void) {
  return NSMaxY(NSScreen.screens.firstObject.frame);
}

SWSnapshot sw_snapshot(SWWindow *owner) {
  NSWindow *window = owner.window;
  NSRect logical = owner.view.bounds;
  NSRect pixels = [owner.view convertRectToBacking:logical];
  NSRect frame = window.frame;
  SWSnapshot result = {0};
  result.width = logical.size.width;
  result.height = logical.size.height;
  result.pixel_width = ceil(pixels.size.width);
  result.pixel_height = ceil(pixels.size.height);
  result.scale = window.backingScaleFactor;
  result.x = frame.origin.x;
  result.y = sw_desktop_top() - NSMaxY(frame);
  result.outer_width = frame.size.width;
  result.outer_height = frame.size.height;
  result.visible = window.visible;
  result.occluded = !(window.occlusionState & NSWindowOcclusionStateVisible);
  result.focused = window.keyWindow;
  result.minimized = window.miniaturized;
  result.maximized = window.zoomed;
  NSAppearanceName appearance =
      [owner.view.effectiveAppearance bestMatchFromAppearancesWithNames:@[
        NSAppearanceNameAqua, NSAppearanceNameDarkAqua
      ]];
  result.dark = [appearance isEqualToString:NSAppearanceNameDarkAqua];
  return result;
}

SWEvent *sw_event(SWWindow *window, int32_t kind) {
  SWEvent *event = [SWEvent new];
  SWEventData data = {0};
  data.window = window.identity;
  data.kind = kind;
  event.data = data;
  return event;
}

@implementation SWView
- (BOOL)isFlipped {
  return YES;
}
- (BOOL)acceptsFirstResponder {
  return YES;
}
- (void)viewDidChangeEffectiveAppearance {
  [super viewDidChangeEffectiveAppearance];
  [self.owner emit:6];
}
@end

@implementation SWWindow
- (void)emit:(int32_t)kind {
  SWEvent *event = sw_event(self, kind);
  SWEventData data = event.data;
  SWSnapshot value = sw_snapshot(self);
  data.x = value.width;
  data.y = value.height;
  data.z = value.pixel_width;
  data.w = value.pixel_height;
  data.scale = value.scale;
  if (kind == 3) {
    data.x = value.x;
    data.y = value.y;
  }
  if (kind == 4)
    data.flags = value.focused;
  if (kind == 5)
    data.flags = value.occluded;
  if (kind == 6)
    data.flags = value.dark;
  event.data = data;
  [self.application enqueue:event];
}
- (BOOL)windowShouldClose:(NSWindow *)sender {
  [self emit:1];
  return NO;
}
- (void)windowDidResize:(NSNotification *)notification {
  [self emit:2];
}
- (void)windowDidChangeBackingProperties:(NSNotification *)notification {
  [self emit:2];
}
- (void)windowDidChangeScreen:(NSNotification *)notification {
  [self emit:2];
}
- (void)windowDidMove:(NSNotification *)notification {
  [self emit:3];
}
- (void)windowDidBecomeKey:(NSNotification *)notification {
  [self emit:4];
}
- (void)windowDidResignKey:(NSNotification *)notification {
  [self emit:4];
}
- (void)windowDidChangeOcclusionState:(NSNotification *)notification {
  [self emit:5];
}
- (void)windowDidMiniaturize:(NSNotification *)notification {
  [self emit:5];
}
- (void)windowDidDeminiaturize:(NSNotification *)notification {
  [self emit:5];
  [self emit:2];
}
@end

void *scoop_window_create(void *application, int64_t identity,
                          const uint8_t *title, int64_t length, double width,
                          double height, int32_t flags) {
  @autoreleasepool {
    SWApplication *app = (__bridge SWApplication *)application;
    SWWindow *owner = [SWWindow new];
    owner.application = app;
    owner.identity = identity;
    NSWindowStyleMask style = NSWindowStyleMaskBorderless;
    if (flags & 1)
      style |= NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
               NSWindowStyleMaskMiniaturizable;
    if (flags & 2)
      style |= NSWindowStyleMaskResizable;
    owner.window =
        [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, width, height)
                                    styleMask:style
                                      backing:NSBackingStoreBuffered
                                        defer:NO];
    if (!owner.window)
      return NULL;
    owner.view = [[SWView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
    owner.view.owner = owner;
    owner.view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    NSView *content = [[NSView alloc] initWithFrame:owner.view.frame];
    [content addSubview:owner.view];
    owner.window.contentView = content;
    owner.window.delegate = owner;
    owner.window.releasedWhenClosed = NO;
    owner.window.title = [[NSString alloc] initWithBytes:title
                                                  length:(NSUInteger)length
                                                encoding:NSUTF8StringEncoding];
    if (flags & 4) {
      owner.transparent = YES;
      owner.window.opaque = NO;
      owner.window.backgroundColor = NSColor.clearColor;
    }
    [owner.window center];
    app.windows[@(identity)] = owner;
    [owner.window makeFirstResponder:owner.view];
    if (flags & 8) {
      if (flags & 16)
        [owner.window makeKeyAndOrderFront:nil];
      else
        [owner.window orderFront:nil];
    }
    [owner emit:2];
    return (__bridge void *)owner;
  }
}

SWSnapshot scoop_window_snapshot(void *pointer) {
  return sw_snapshot((__bridge SWWindow *)pointer);
}

void scoop_window_destroy(void *pointer) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  if (owner.attentionRequest)
    [NSApp cancelUserAttentionRequest:owner.attentionRequest];
  owner.window.delegate = nil;
  owner.view.owner = nil;
  [owner.window orderOut:nil];
  [owner.window close];
  [owner.application.windows removeObjectForKey:@(owner.identity)];
}

void *scoop_window_ns_window(void *pointer) {
  return (__bridge void *)((__bridge SWWindow *)pointer).window;
}
void *scoop_window_ns_view(void *pointer) {
  return (__bridge void *)((__bridge SWWindow *)pointer).view;
}

void scoop_window_title(void *pointer, const uint8_t *text, int64_t length) {
  ((__bridge SWWindow *)pointer).window.title =
      [[NSString alloc] initWithBytes:text
                               length:(NSUInteger)length
                             encoding:NSUTF8StringEncoding];
}
void scoop_window_size(void *pointer, double width, double height) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  [owner.window setContentSize:NSMakeSize(width, height)];
  [owner emit:2];
}
void scoop_window_visible(void *pointer, int32_t value) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  if (value)
    [owner.window orderFront:nil];
  else
    [owner.window orderOut:nil];
  [owner emit:5];
}

void scoop_window_focus(void *pointer) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  [NSApp activate];
  [owner.window makeKeyAndOrderFront:nil];
}
