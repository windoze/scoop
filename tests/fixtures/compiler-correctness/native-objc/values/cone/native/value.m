#import <AppKit/AppKit.h>
#include <stdint.h>

#if !OBJC_ONLY || defined(C_ONLY) || !__has_feature(objc_arc)
#error incorrect Objective-C configuration
#endif

extern int scoop_objc_from_c(void);

@interface ScoopNativeProbe : NSObject
@property(nonatomic, copy) NSString *text;
@end

@implementation ScoopNativeProbe
@end

int32_t scoop_objc_value(void) {
    __weak ScoopNativeProbe *weak;
    int32_t result = 0;
    @autoreleasepool {
        ScoopNativeProbe *probe = [ScoopNativeProbe new];
        probe.text = @"Scoop";
        weak = probe;
        NSView *view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, OBJC_VALUE, 24)];
        if ([probe respondsToSelector:@selector(text)] && weak == probe) {
            result = (int32_t)probe.text.length + (int32_t)view.frame.size.width +
                     scoop_objc_from_c();
        }
    }
    return weak == nil ? result : -1;
}
