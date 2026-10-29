#import "window.h"
#include <float.h>

static void set_style(SWWindow *owner, NSWindowStyleMask style) {
  if (owner.window.styleMask == style)
    return;
  owner.window.styleMask = style;
  // AppKit can replace the first responder while changing the frame style.
  [owner.window makeFirstResponder:owner.view];
}

void scoop_window_position(void *pointer, double x, double y) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  [owner.window setFrameTopLeftPoint:NSMakePoint(x, sw_desktop_top() - y)];
}

void scoop_window_constraints(void *pointer, double min_width,
                              double min_height, double max_width,
                              double max_height) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  NSWindow *window = owner.window;
  window.contentMinSize = NSMakeSize(min_width, min_height);
  window.contentMaxSize = NSMakeSize(max_width < 0 ? DBL_MAX : max_width,
                                     max_height < 0 ? DBL_MAX : max_height);
  NSSize size = owner.view.bounds.size;
  size.width = MAX(min_width, MIN(window.contentMaxSize.width, size.width));
  size.height = MAX(min_height, MIN(window.contentMaxSize.height, size.height));
  if (!NSEqualSizes(size, owner.view.bounds.size))
    [window setContentSize:size];
}

void scoop_window_resize_increments(void *pointer, double width,
                                    double height) {
  ((__bridge SWWindow *)pointer).window.contentResizeIncrements =
      NSMakeSize(width, height);
}

void scoop_window_resizable(void *pointer, int32_t value) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  NSWindowStyleMask style = owner.window.styleMask;
  set_style(owner, value ? style | NSWindowStyleMaskResizable
                         : style & ~NSWindowStyleMaskResizable);
}

void scoop_window_decorations(void *pointer, int32_t value) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  NSWindowStyleMask decorations = NSWindowStyleMaskTitled |
                                  NSWindowStyleMaskClosable |
                                  NSWindowStyleMaskMiniaturizable;
  NSWindowStyleMask style = owner.window.styleMask;
  set_style(owner, value ? style | decorations : style & ~decorations);
}

void scoop_window_buttons(void *pointer, int32_t close, int32_t minimize,
                          int32_t maximize) {
  NSWindow *window = ((__bridge SWWindow *)pointer).window;
  [window standardWindowButton:NSWindowCloseButton].enabled = close != 0;
  [window standardWindowButton:NSWindowMiniaturizeButton].enabled =
      minimize != 0;
  [window standardWindowButton:NSWindowZoomButton].enabled = maximize != 0;
}

void scoop_window_level(void *pointer, int32_t level) {
  ((__bridge SWWindow *)pointer).window.level =
      level > 0   ? NSFloatingWindowLevel
      : level < 0 ? NSNormalWindowLevel - 1
                  : NSNormalWindowLevel;
}

void scoop_window_minimized(void *pointer, int32_t value) {
  NSWindow *window = ((__bridge SWWindow *)pointer).window;
  if (value && !window.miniaturized)
    [window miniaturize:nil];
  else if (!value && window.miniaturized)
    [window deminiaturize:nil];
}

void scoop_window_maximized(void *pointer, int32_t value) {
  NSWindow *window = ((__bridge SWWindow *)pointer).window;
  if ((window.zoomed != NO) != (value != 0))
    [window zoom:nil];
}

void scoop_window_theme(void *pointer, int32_t value) {
  NSWindow *window = ((__bridge SWWindow *)pointer).window;
  window.appearance =
      value < 0 ? nil
                : [NSAppearance appearanceNamed:value ? NSAppearanceNameDarkAqua
                                                      : NSAppearanceNameAqua];
}

static void background(SWWindow *owner) {
  owner.window.opaque = !(owner.transparent || owner.effect);
  owner.window.backgroundColor =
      owner.window.opaque ? NSColor.windowBackgroundColor : NSColor.clearColor;
}

void scoop_window_transparent(void *pointer, int32_t value) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  owner.transparent = value != 0;
  background(owner);
}

void scoop_window_blur(void *pointer, int32_t value) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  if (value && !owner.effect) {
    owner.effect = [[NSVisualEffectView alloc]
        initWithFrame:owner.window.contentView.bounds];
    owner.effect.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    owner.effect.material = NSVisualEffectMaterialUnderWindowBackground;
    owner.effect.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    owner.effect.state = NSVisualEffectStateFollowsWindowActiveState;
    [owner.window.contentView addSubview:owner.effect
                              positioned:NSWindowBelow
                              relativeTo:owner.view];
  } else if (!value && owner.effect) {
    [owner.effect removeFromSuperview];
    owner.effect = nil;
  }
  background(owner);
}

void scoop_window_content_protection(void *pointer, int32_t value) {
  ((__bridge SWWindow *)pointer).window.sharingType =
      value ? NSWindowSharingNone : NSWindowSharingReadOnly;
}

void scoop_window_attention(void *pointer, int32_t kind) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  if (owner.attentionRequest)
    [NSApp cancelUserAttentionRequest:owner.attentionRequest];
  owner.attentionRequest =
      kind < 0 ? 0
               : [NSApp requestUserAttention:kind ? NSCriticalRequest
                                                  : NSInformationalRequest];
}

void scoop_window_titlebar(void *pointer, int32_t flags) {
  SWWindow *owner = (__bridge SWWindow *)pointer;
  NSWindow *window = owner.window;
  window.titlebarAppearsTransparent = (flags & 1) != 0;
  window.titleVisibility =
      (flags & 2) ? NSWindowTitleHidden : NSWindowTitleVisible;
  NSWindowStyleMask style = window.styleMask;
  set_style(owner, (flags & 4) ? style | NSWindowStyleMaskFullSizeContentView
                               : style & ~NSWindowStyleMaskFullSizeContentView);
}

typedef struct {
  const uint8_t *data;
  int64_t length;
} SWBundleIdentifier;
SWBundleIdentifier scoop_window_bundle_identifier(void) {
  NSString *name = NSBundle.mainBundle.bundleIdentifier;
  SWBundleIdentifier result = {
      (const uint8_t *)name.UTF8String,
      (int64_t)[name lengthOfBytesUsingEncoding:NSUTF8StringEncoding]};
  return result;
}
