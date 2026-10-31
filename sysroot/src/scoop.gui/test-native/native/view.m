#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#include <stdio.h>

@interface SGUTestView : NSObject
@property(nonatomic, strong) NSView *view;
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) NSView *contentView;
@property(nonatomic, strong) NSView *superview;
@property(nonatomic, strong) CALayer *layer;
@property(nonatomic) BOOL wantsLayer;
@end
@implementation SGUTestView
@end

void *scoop_gui_test_view_create(void *pointer, bool customLayer) {
  SGUTestView *probe = [SGUTestView new];
  probe.view = (__bridge NSView *)pointer;
  if (customLayer) {
    probe.view.layer = [CALayer layer];
    probe.view.wantsLayer = YES;
  }
  probe.window = probe.view.window;
  probe.contentView = probe.window.contentView;
  probe.superview = probe.view.superview;
  probe.layer = probe.view.layer;
  probe.wantsLayer = probe.view.wantsLayer;
  return (__bridge_retained void *)probe;
}
bool scoop_gui_test_view_check(void *pointer, int32_t width, int32_t height,
                               double scale) {
  SGUTestView *probe = (__bridge SGUTestView *)pointer;
  if (![probe.view.layer isKindOfClass:CAMetalLayer.class]) {
    fprintf(stderr, "input view does not have a CAMetalLayer\n");
    return false;
  }
  CAMetalLayer *layer = (CAMetalLayer *)probe.view.layer;
  bool hierarchy = probe.view.window == probe.window &&
                   probe.window.contentView == probe.contentView &&
                   probe.view.superview == probe.superview &&
                   [probe.view isDescendantOf:probe.contentView];
  bool matches = hierarchy && probe.view.isFlipped &&
                 [probe.view conformsToProtocol:@protocol(NSTextInputClient)] &&
                 layer.drawableSize.width == width &&
                 layer.drawableSize.height == height &&
                 layer.contentsScale == scale && layer.device != nil &&
                 layer.pixelFormat == MTLPixelFormatBGRA8Unorm &&
                 !layer.framebufferOnly;
  if (!matches)
    fprintf(stderr,
            "input view: hierarchy=%d flipped=%d textClient=%d device=%d "
            "pixelFormat=%lu framebufferOnly=%d; expected=%dx%d@%g "
            "actual=%gx%g@%g\n",
            hierarchy, probe.view.isFlipped,
            [probe.view conformsToProtocol:@protocol(NSTextInputClient)],
            layer.device != nil, (unsigned long)layer.pixelFormat,
            layer.framebufferOnly, (int)width, (int)height, scale,
            layer.drawableSize.width, layer.drawableSize.height,
            layer.contentsScale);
  return matches;
}
bool scoop_gui_test_view_restored(void *pointer) {
  SGUTestView *probe = (__bridge SGUTestView *)pointer;
  // AppKit may create an automatic backing layer after restoring a nil layer.
  bool restoredLayer =
      probe.layer ? probe.view.layer == probe.layer
                  : ![probe.view.layer isKindOfClass:CAMetalLayer.class];
  bool restored = restoredLayer && probe.view.wantsLayer == probe.wantsLayer;
  if (!restored)
    fprintf(stderr,
            "restored view: layer=%p expected=%p wantsLayer=%d expected=%d\n",
            (__bridge void *)probe.view.layer, (__bridge void *)probe.layer,
            probe.view.wantsLayer, probe.wantsLayer);
  return restored;
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
