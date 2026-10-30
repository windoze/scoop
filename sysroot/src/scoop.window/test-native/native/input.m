#import "../../native/darwin/window.h"
#import <CoreGraphics/CoreGraphics.h>

static NSEvent *key_event(SWView *view, NSEventType type, unsigned short code,
                          NSString *text, NSEventModifierFlags flags,
                          BOOL repeat) {
  return [NSEvent keyEventWithType:type
                          location:NSZeroPoint
                     modifierFlags:flags
                         timestamp:0
                      windowNumber:view.window.windowNumber
                           context:nil
                        characters:text
       charactersIgnoringModifiers:text
                         isARepeat:repeat
                           keyCode:code];
}

static void cancelled_sequences(SWView *view) {
  NSPoint point = [view convertPoint:NSMakePoint(20, 30) toView:nil];
  NSEvent *down = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDown
                                     location:point
                                modifierFlags:0
                                    timestamp:0
                                 windowNumber:view.window.windowNumber
                                      context:nil
                                  eventNumber:10
                                   clickCount:7
                                     pressure:1];
  NSEvent *up = [NSEvent mouseEventWithType:NSEventTypeLeftMouseUp
                                   location:point
                              modifierFlags:0
                                  timestamp:0
                               windowNumber:view.window.windowNumber
                                    context:nil
                                eventNumber:11
                                 clickCount:7
                                   pressure:0];
  NSEvent *left =
      [NSEvent enterExitEventWithType:NSEventTypeMouseExited
                             location:[view convertPoint:NSMakePoint(-17, -19)
                                                  toView:nil]
                        modifierFlags:0
                            timestamp:0
                         windowNumber:view.window.windowNumber
                              context:nil
                          eventNumber:12
                       trackingNumber:0
                             userData:NULL];
  [view mouseDown:down];
  [view mouseExited:left];
  [view resetInput];
  [view resetInput];
  [view mouseUp:up];
  [view mouseDown:down];
  NSNotification *inactive = [NSNotification
      notificationWithName:NSApplicationDidResignActiveNotification
                    object:NSApp];
  [view.owner.application applicationDidResignActive:inactive];
  [view.owner.application applicationDidResignActive:inactive];
  [view mouseUp:up];
  [view mouseDown:down];
  [NSNotificationCenter.defaultCenter
      postNotificationName:NSMenuDidBeginTrackingNotification
                    object:[NSMenu new]];
  [view mouseUp:up];
  [view mouseDown:down];
  [view mouseUp:up];
  [NSNotificationCenter.defaultCenter
      postNotificationName:NSMenuDidBeginTrackingNotification
                    object:[NSMenu new]];
}

int32_t scoop_window_test_input(void *pointer) {
  SWView *view = (__bridge SWView *)pointer;
  [view keyDown:key_event(view, NSEventTypeKeyDown, 0, @"a", 0, NO)];
  [view keyDown:key_event(view, NSEventTypeKeyDown, 0, @"a", 0, YES)];
  [view keyUp:key_event(view, NSEventTypeKeyUp, 0, @"a", 0, NO)];
  [view keyDown:key_event(view, NSEventTypeKeyDown, 0xffff, @"λ", 0, NO)];
  [view keyUp:key_event(view, NSEventTypeKeyUp, 0xffff, @"λ", 0, NO)];
  [view keyDown:key_event(view, NSEventTypeKeyDown, 8, @"c",
                          NSEventModifierFlagCommand, NO)];
  [view keyUp:key_event(view, NSEventTypeKeyUp, 8, @"c",
                        NSEventModifierFlagCommand, NO)];
  [view flagsChanged:key_event(view, NSEventTypeFlagsChanged, 56, @"",
                               NSEventModifierFlagShift | 2, NO)];
  [view flagsChanged:key_event(view, NSEventTypeFlagsChanged, 60, @"",
                               NSEventModifierFlagShift | 6, NO)];
  [view flagsChanged:key_event(view, NSEventTypeFlagsChanged, 56, @"",
                               NSEventModifierFlagShift | 4, NO)];
  [view flagsChanged:key_event(view, NSEventTypeFlagsChanged, 60, @"", 0, NO)];
  [view keyDown:key_event(view, NSEventTypeKeyDown, 11, @"b", 0, NO)];
  [view flagsChanged:key_event(view, NSEventTypeFlagsChanged, 57, @"",
                               NSEventModifierFlagCapsLock, NO)];
  [view resetInput];

  NSPoint point = [view convertPoint:NSMakePoint(20, 30) toView:nil];
  NSEvent *enter = [NSEvent enterExitEventWithType:NSEventTypeMouseEntered
                                          location:point
                                     modifierFlags:0
                                         timestamp:0
                                      windowNumber:view.window.windowNumber
                                           context:nil
                                       eventNumber:1
                                    trackingNumber:0
                                          userData:NULL];
  [view mouseEntered:enter];
  if (view.window.keyWindow && !view.cursorVisible && !view.cursorHidden)
    return -2;
  NSEvent *move = [NSEvent mouseEventWithType:NSEventTypeMouseMoved
                                     location:point
                                modifierFlags:0
                                    timestamp:0
                                 windowNumber:view.window.windowNumber
                                      context:nil
                                  eventNumber:2
                                   clickCount:0
                                     pressure:0];
  [view mouseMoved:move];
  NSEvent *down = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDown
                                     location:point
                                modifierFlags:0
                                    timestamp:0
                                 windowNumber:view.window.windowNumber
                                      context:nil
                                  eventNumber:3
                                   clickCount:2
                                     pressure:1];
  [view mouseDown:down];
  NSEvent *up = [NSEvent mouseEventWithType:NSEventTypeLeftMouseUp
                                   location:point
                              modifierFlags:0
                                  timestamp:0
                               windowNumber:view.window.windowNumber
                                    context:nil
                                eventNumber:4
                                 clickCount:2
                                   pressure:0];
  [view mouseUp:up];
  for (int unit = 0; unit < 3; ++unit) {
    CGEventRef scroll = CGEventCreateScrollWheelEvent2(
        NULL, unit == 0 ? kCGScrollEventUnitLine : kCGScrollEventUnitPixel, 2,
        5, 3, 0);
    if (!scroll)
      return -1;
    if (unit == 1)
      CGEventSetIntegerValueField(scroll, kCGScrollWheelEventScrollPhase,
                                  kCGScrollPhaseBegan);
    if (unit == 2)
      CGEventSetIntegerValueField(scroll, kCGScrollWheelEventMomentumPhase,
                                  kCGMomentumScrollPhaseBegin);
    [view scrollWheel:[NSEvent eventWithCGEvent:scroll]];
    CFRelease(scroll);
  }
  NSEvent *exit = [NSEvent enterExitEventWithType:NSEventTypeMouseExited
                                         location:point
                                    modifierFlags:0
                                        timestamp:0
                                     windowNumber:view.window.windowNumber
                                          context:nil
                                      eventNumber:5
                                   trackingNumber:0
                                         userData:NULL];
  [view mouseExited:exit];
  cancelled_sequences(view);
  return view.cursorHidden ? -3 : 0;
}
