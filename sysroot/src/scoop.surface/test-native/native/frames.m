#import <AppKit/AppKit.h>
#import <QuartzCore/CAMetalDisplayLink.h>
#import <objc/runtime.h>

// Test-only observation of the actual renderer, without adding production
// counters or a clock.
@interface SSUTestFrames : NSObject
@property(nonatomic, weak) NSView *view;
@property(nonatomic, weak) NSObject *surface;
@property(nonatomic) int64_t callbacks;
@property(nonatomic) int64_t presentations;
@end
@implementation SSUTestFrames
@end

static NSMapTable<NSView *, SSUTestFrames *> *observed;
static IMP demandImpl, callbackImpl, commandImpl;

static SSUTestFrames *framesFor(NSObject *surface) {
  NSView *view = [surface valueForKey:@"view"];
  return view ? [observed objectForKey:view] : nil;
}
static void observeDemand(NSObject *surface, SEL selector) {
  ((void (*)(id, SEL))demandImpl)(surface, selector);
  SSUTestFrames *frames = framesFor(surface);
  frames.surface = surface;
}
static void observeCallback(NSObject *surface, SEL selector,
                            CAMetalDisplayLink *link,
                            CAMetalDisplayLinkUpdate *update) {
  SSUTestFrames *frames = framesFor(surface);
  frames.callbacks += 1;
  ((void (*)(id, SEL, id, id))callbackImpl)(surface, selector, link, update);
}
static void observeCommand(NSObject *surface, SEL selector, id command) {
  ((void (*)(id, SEL, id))commandImpl)(surface, selector, command);
  if (command) {
    SSUTestFrames *frames = framesFor(surface);
    frames.presentations += 1;
  }
}
static bool installObservers(void) {
  if (observed)
    return true;
  Class surface = NSClassFromString(@"SSUSurface");
  Method demand =
      class_getInstanceMethod(surface, NSSelectorFromString(@"updateDemand"));
  Method callback = class_getInstanceMethod(
      surface, NSSelectorFromString(@"metalDisplayLink:needsUpdate:"));
  Method command = class_getInstanceMethod(
      surface, NSSelectorFromString(@"setLastCommand:"));
  if (!demand || !callback || !command)
    return false;
  observed = [NSMapTable weakToStrongObjectsMapTable];
  demandImpl = method_setImplementation(demand, (IMP)observeDemand);
  callbackImpl = method_setImplementation(callback, (IMP)observeCallback);
  commandImpl = method_setImplementation(command, (IMP)observeCommand);
  return true;
}

typedef struct {
  int32_t attached, required, paused;
  int64_t callbacks, presentations;
} SSUTestFrameSnapshot;

void *scoop_surface_test_frames_create(void *pointer) {
  if (!NSThread.isMainThread || !installObservers())
    return NULL;
  NSView *view = (__bridge NSView *)pointer;
  if ([observed objectForKey:view])
    return NULL;
  SSUTestFrames *frames = [SSUTestFrames new];
  frames.view = view;
  [observed setObject:frames forKey:view];
  return (__bridge_retained void *)frames;
}
SSUTestFrameSnapshot scoop_surface_test_frames_snapshot(void *pointer) {
  SSUTestFrames *frames = (__bridge SSUTestFrames *)pointer;
  NSObject *surface = frames.surface;
  BOOL closed = !surface || [[surface valueForKey:@"closed"] boolValue];
  CAMetalDisplayLink *link = closed ? nil : [surface valueForKey:@"link"];
  return (SSUTestFrameSnapshot){
      !closed, !closed && [[surface valueForKey:@"required"] boolValue],
      !link || link.paused, frames.callbacks, frames.presentations};
}
void scoop_surface_test_frames_drop(void *pointer) {
  SSUTestFrames *frames = (__bridge_transfer SSUTestFrames *)pointer;
  if (frames.view && [observed objectForKey:frames.view] == frames)
    [observed removeObjectForKey:frames.view];
}
