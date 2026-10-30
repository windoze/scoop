#import "window.h"

static void pointer(SWView *view, NSEvent *native, int32_t kind) {
  sw_modifiers(view, native.modifierFlags);
  SWEvent *event = sw_event(view.owner, SWPointer);
  SWEventData data = event.data;
  NSPoint logical = [view convertPoint:native.locationInWindow fromView:nil];
  data.flags = kind;
  data.x = logical.x;
  data.y = logical.y;
  data.scale = view.window.backingScaleFactor;
  // Backing surface points may use a different origin; both payload positions
  // use the content view's top-left origin and the scale from this event.
  data.z = logical.x * data.scale;
  data.w = logical.y * data.scale;
  if (kind == 2) {
    view.pointerInside = NSMouseInRect(logical, view.bounds, view.flipped);
    [view updateCursor];
  }
  data.b = (int64_t)native.modifierFlags;
  if (kind == 3 || kind == 4) {
    data.a = native.buttonNumber;
    data.c = native.clickCount;
  }
  if (kind == 5 || kind == 6) {
    data.a = (int64_t)native.phase;
    data.c = (int64_t)native.momentumPhase;
    data.dx = native.scrollingDeltaX * (kind == 6 ? data.scale : 1);
    data.dy = native.scrollingDeltaY * (kind == 6 ? data.scale : 1);
  }
  event.data = data;
  [view.owner.application enqueue:event];
}

@implementation SWView (Pointer)
- (void)updateTrackingAreas {
  [super updateTrackingAreas];
  if (self.tracking)
    [self removeTrackingArea:self.tracking];
  self.tracking = [[NSTrackingArea alloc]
      initWithRect:NSZeroRect
           options:NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved |
                   NSTrackingActiveAlways | NSTrackingInVisibleRect |
                   NSTrackingCursorUpdate
             owner:self
          userInfo:nil];
  [self addTrackingArea:self.tracking];
}
- (void)resetCursorRects {
  [self addCursorRect:self.bounds cursor:self.cursor ?: NSCursor.arrowCursor];
}
- (void)cursorUpdate:(NSEvent *)event {
  [self updateCursor];
}
- (void)updateCursor {
  BOOL hidden =
      self.pointerInside && !self.cursorVisible && self.window.keyWindow;
  if (hidden != self.cursorHidden) {
    if (hidden)
      [NSCursor hide];
    else
      [NSCursor unhide];
    self.cursorHidden = hidden;
  }
  if (self.pointerInside)
    [self.cursor set];
}
- (void)mouseEntered:(NSEvent *)event {
  self.pointerInside = YES;
  [self updateCursor];
  pointer(self, event, 0);
}
- (void)mouseExited:(NSEvent *)event {
  self.pointerInside = NO;
  [self updateCursor];
  pointer(self, event, 1);
}
- (void)mouseMoved:(NSEvent *)event {
  pointer(self, event, 2);
}
- (void)mouseDragged:(NSEvent *)event {
  pointer(self, event, 2);
}
- (void)rightMouseDragged:(NSEvent *)event {
  pointer(self, event, 2);
}
- (void)otherMouseDragged:(NSEvent *)event {
  pointer(self, event, 2);
}
- (void)mouseDown:(NSEvent *)event {
  pointer(self, event, 3);
}
- (void)rightMouseDown:(NSEvent *)event {
  pointer(self, event, 3);
}
- (void)otherMouseDown:(NSEvent *)event {
  pointer(self, event, 3);
}
- (void)mouseUp:(NSEvent *)event {
  pointer(self, event, 4);
}
- (void)rightMouseUp:(NSEvent *)event {
  pointer(self, event, 4);
}
- (void)otherMouseUp:(NSEvent *)event {
  pointer(self, event, 4);
}
- (void)scrollWheel:(NSEvent *)event {
  pointer(self, event, event.hasPreciseScrollingDeltas ? 6 : 5);
}
@end

int64_t scoop_window_cursor_icon(void *pointer, int32_t icon) {
  SWView *view = ((__bridge SWWindow *)pointer).view;
  switch (icon) {
  case 1:
    view.cursor = NSCursor.IBeamCursor;
    break;
  case 2:
    view.cursor = NSCursor.crosshairCursor;
    break;
  case 3:
    view.cursor = NSCursor.pointingHandCursor;
    break;
  case 4:
    view.cursor = NSCursor.openHandCursor;
    break;
  case 5:
    view.cursor = NSCursor.closedHandCursor;
    break;
  case 6:
    view.cursor = NSCursor.operationNotAllowedCursor;
    break;
  case 7:
    view.cursor = NSCursor.resizeLeftRightCursor;
    break;
  case 8:
    view.cursor = NSCursor.resizeUpDownCursor;
    break;
  default:
    view.cursor = NSCursor.arrowCursor;
    break;
  }
  [view.window invalidateCursorRectsForView:view];
  [view updateCursor];
  return 0;
}

int64_t scoop_window_cursor_visible(void *pointer, int32_t visible) {
  SWView *view = ((__bridge SWWindow *)pointer).view;
  view.cursorVisible = visible != 0;
  [view updateCursor];
  return 0;
}
