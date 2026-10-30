#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>

@interface SGUTestView : NSObject
@property(nonatomic, strong) NSView *view;
@property(nonatomic, strong) CALayer *layer;
@property(nonatomic) BOOL wantsLayer;
@end
@implementation SGUTestView
@end

void *scoop_gui_test_view_create(void *pointer) {
  SGUTestView *probe = [SGUTestView new];
  probe.view = (__bridge NSView *)pointer;
  probe.layer = probe.view.layer;
  probe.wantsLayer = probe.view.wantsLayer;
  return (__bridge_retained void *)probe;
}
bool scoop_gui_test_view_check(void *pointer, int32_t width, int32_t height,
                               double scale) {
  SGUTestView *probe = (__bridge SGUTestView *)pointer;
  if (![probe.view.layer isKindOfClass:CAMetalLayer.class])
    return false;
  CAMetalLayer *layer = (CAMetalLayer *)probe.view.layer;
  return probe.view.window.contentView == probe.view && probe.view.isFlipped &&
         [probe.view conformsToProtocol:@protocol(NSTextInputClient)] &&
         layer.drawableSize.width == width &&
         layer.drawableSize.height == height && layer.contentsScale == scale &&
         layer.device != nil && layer.pixelFormat == MTLPixelFormatBGRA8Unorm &&
         !layer.framebufferOnly;
}
bool scoop_gui_test_view_restored(void *pointer) {
  SGUTestView *probe = (__bridge SGUTestView *)pointer;
  return probe.view.layer == probe.layer &&
         probe.view.wantsLayer == probe.wantsLayer;
}
void scoop_gui_test_view_drop(void *pointer) {
  SGUTestView *probe = (__bridge_transfer SGUTestView *)pointer;
  (void)probe;
}

bool scoop_gui_test_view_button(void *pointer, double x, double y,
                                bool pressed) {
  @autoreleasepool {
    @try {
      NSView *view = ((__bridge SGUTestView *)pointer).view;
      NSEvent *event = [NSEvent
          mouseEventWithType:pressed ? NSEventTypeLeftMouseDown
                                     : NSEventTypeLeftMouseUp
                    location:[view convertPoint:NSMakePoint(x, y) toView:nil]
               modifierFlags:0
                   timestamp:NSProcessInfo.processInfo.systemUptime
                windowNumber:view.window.windowNumber
                     context:nil
                 eventNumber:1
                  clickCount:1
                    pressure:pressed ? 1.0 : 0.0];
      if (pressed)
        [view mouseDown:event];
      else
        [view mouseUp:event];
      return true;
    } @catch (NSException *exception) {
      (void)exception;
      return false;
    }
  }
}

bool scoop_gui_test_view_text(void *pointer, bool commit) {
  @autoreleasepool {
    @try {
      id<NSTextInputClient> client =
          (id<NSTextInputClient>)((__bridge SGUTestView *)pointer).view;
      NSRange absent = NSMakeRange(NSNotFound, 0);
      if (commit)
        [client insertText:@"A" replacementRange:absent];
      else
        [client setMarkedText:@"中😀"
                selectedRange:NSMakeRange(1, 2)
             replacementRange:absent];
      return true;
    } @catch (NSException *exception) {
      (void)exception;
      return false;
    }
  }
}

bool scoop_gui_test_view_caret(void *pointer, double x, double y, double width,
                               double height) {
  @autoreleasepool {
    @try {
      NSView *view = ((__bridge SGUTestView *)pointer).view;
      if (!view.window.keyWindow || view.window.firstResponder != view)
        return false;
      id<NSTextInputClient> client = (id<NSTextInputClient>)view;
      NSRange actual = NSMakeRange(0, 0);
      NSRect result =
          [client firstRectForCharacterRange:NSMakeRange(NSNotFound, 0)
                                 actualRange:&actual];
      NSRect expected = [view.window
          convertRectToScreen:[view convertRect:NSMakeRect(x, y, width, height)
                                         toView:nil]];
      return NSEqualRects(result, expected) && actual.location == NSNotFound;
    } @catch (NSException *exception) {
      (void)exception;
      return false;
    }
  }
}
