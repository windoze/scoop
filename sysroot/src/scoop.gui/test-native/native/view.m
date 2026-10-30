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
