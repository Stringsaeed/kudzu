#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <objc/runtime.h>

// A real NSView owns the gesture: IPC cannot reliably recover the mouse-down NSEvent.
@class KZPermissionGuide;
@interface KZAppDragView : NSView <NSDraggingSource>
@property(nonatomic, strong) NSEvent *downEvent;
@property(nonatomic, weak) KZPermissionGuide *guide;
@end

@interface KZGuideDrawing : NSView
@property NSRect destination;
@property NSPoint source;
@property CGFloat progress;
@property(nonatomic, strong) NSImage *icon;
@end

@interface KZPermissionGuide : NSObject
@property(nonatomic, weak) NSWindow *window;
@property(nonatomic, strong) KZAppDragView *dragView;
@property(nonatomic, strong) NSPanel *overlay;
@property(nonatomic, strong) NSTimer *timer;
@property NSUInteger generation;
- (void)stop;
- (void)showBesideSettings:(NSUInteger)attempt generation:(NSUInteger)generation;
@end

static NSURL *applicationURL(void) {
    NSURL *url = NSBundle.mainBundle.bundleURL;
    return [url.pathExtension.lowercaseString isEqualToString:@"app"] ? url : nil;
}

@implementation KZAppDragView
- (BOOL)acceptsFirstMouse:(NSEvent *)event { (void)event; return YES; }
- (void)resetCursorRects { [self addCursorRect:self.bounds cursor:NSCursor.openHandCursor]; }
- (void)mouseDown:(NSEvent *)event { self.downEvent = event; }
- (void)mouseUp:(NSEvent *)event {
    (void)event;
    if (self.downEvent && applicationURL()) {
        [self.guide stop];
        [NSWorkspace.sharedWorkspace activateFileViewerSelectingURLs:@[applicationURL()]];
    }
    self.downEvent = nil;
}
- (void)mouseDragged:(NSEvent *)event {
    if (!self.downEvent || !applicationURL()) return;
    if (hypot(event.locationInWindow.x - self.downEvent.locationInWindow.x,
              event.locationInWindow.y - self.downEvent.locationInWindow.y) < 4) return;
    [self.guide stop];
    NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:applicationURL()];
    NSImage *icon = [NSWorkspace.sharedWorkspace iconForFile:applicationURL().path];
    [item setDraggingFrame:self.bounds contents:icon];
    NSDraggingSession *session = [self beginDraggingSessionWithItems:@[item] event:self.downEvent source:self];
    session.animatesToStartingPositionsOnCancelOrFail = YES;
    self.downEvent = nil;
}
- (NSDragOperation)draggingSession:(NSDraggingSession *)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context {
    (void)session; (void)context;
    return NSDragOperationCopy;
}
@end

@implementation KZGuideDrawing
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    CGFloat phase = self.progress;
    CGFloat fade = MIN(1, MIN(phase * 10, (1 - phase) * 10));
    [[NSColor.controlAccentColor colorWithAlphaComponent:0.65 * fade] setStroke];
    NSBezierPath *outline = [NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.destination, -3, -3) xRadius:14 yRadius:14];
    outline.lineWidth = 3;
    [outline stroke];
    // Point into the Settings window, without assuming the location of a specific OS control.
    NSPoint end = NSMakePoint(NSMidX(self.destination) + self.destination.size.width * 0.15, NSMidY(self.destination));
    NSPoint bend = NSMakePoint((self.source.x + end.x) / 2, MAX(self.source.y, end.y) + 65);
    NSBezierPath *path = [NSBezierPath bezierPath];
    [path moveToPoint:self.source];
    [path curveToPoint:end controlPoint1:bend controlPoint2:bend];
    CGFloat dash[] = {4, 6};
    [path setLineDash:dash count:2 phase:0];
    path.lineWidth = 1.5;
    [path stroke];
    CGFloat t = MIN(1, MAX(0, (phase - 0.12) / 0.65));
    t = t * t * (3 - 2 * t);
    CGFloat u = 1 - t;
    NSPoint point = NSMakePoint(u*u*u*self.source.x + 3*u*t*bend.x + t*t*t*end.x,
                               u*u*u*self.source.y + 3*u*t*bend.y + t*t*t*end.y);
    [self.icon drawInRect:NSMakeRect(point.x - 26, point.y - 26, 52, 52)
                fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:fade * 0.85];
}
@end

// Window metadata only: no screen capture and no Accessibility requirement.
static NSRect settingsFrame(void) {
    NSRunningApplication *settings = [NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.systempreferences"].firstObject;
    if (!settings) return NSZeroRect;
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    for (NSDictionary *info in windows) {
        if ([info[(__bridge NSString *)kCGWindowOwnerPID] intValue] != settings.processIdentifier ||
            [info[(__bridge NSString *)kCGWindowLayer] intValue] != 0) continue;
        CGRect bounds;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)info[(__bridge NSString *)kCGWindowBounds], &bounds)) continue;
        if (bounds.size.width < 400 || bounds.size.height < 300) continue;
        CGFloat desktopTop = NSScreen.screens.firstObject.frame.size.height;
        return NSMakeRect(bounds.origin.x, desktopTop - CGRectGetMaxY(bounds), bounds.size.width, bounds.size.height);
    }
    return NSZeroRect;
}

@implementation KZPermissionGuide
- (void)stop {
    self.generation++;
    [self.timer invalidate];
    self.timer = nil;
    [self.overlay orderOut:nil];
    self.overlay = nil;
}
- (void)showBesideSettings:(NSUInteger)attempt generation:(NSUInteger)generation {
    if (generation != self.generation || !self.window.visible) return;
    NSRect destination = settingsFrame();
    if (NSIsEmptyRect(destination)) {
        if (attempt < 12) {
            __weak KZPermissionGuide *weakSelf = self;
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 200 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
                [weakSelf showBesideSettings:attempt + 1 generation:generation];
            });
        }
        return;
    }
    NSScreen *screen = self.window.screen ?: NSScreen.mainScreen;
    for (NSScreen *candidate in NSScreen.screens) {
        if (NSPointInRect(NSMakePoint(NSMidX(destination), NSMidY(destination)), candidate.frame)) { screen = candidate; break; }
    }
    NSRect available = screen.visibleFrame;
    NSRect frame = self.window.frame;
    CGFloat x = NSMinX(destination) - frame.size.width - 18;
    if (x < NSMinX(available) + 12) x = NSMaxX(destination) + 18;
    x = MAX(NSMinX(available) + 12, MIN(x, NSMaxX(available) - frame.size.width - 12));
    CGFloat y = MAX(NSMinY(available) + 12, MIN(NSMaxY(destination) - frame.size.height, NSMaxY(available) - frame.size.height - 12));
    [self.window setFrameOrigin:NSMakePoint(x, y)];
    // On a small display a guide stays useful, but an overlapping ghost would be confusing.
    if (NSIntersectsRect(self.window.frame, destination) || NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion) return;
    if (!self.dragView || !applicationURL()) return;
    NSRect sourceRect = [self.window convertRectToScreen:[self.dragView convertRect:self.dragView.bounds toView:nil]];
    NSRect canvas = NSInsetRect(NSUnionRect(self.window.frame, destination), -80, -80);
    NSPanel *overlay = [[NSPanel alloc] initWithContentRect:canvas styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    overlay.opaque = NO;
    overlay.backgroundColor = NSColor.clearColor;
    overlay.hasShadow = NO;
    overlay.ignoresMouseEvents = YES;
    overlay.level = NSFloatingWindowLevel;
    overlay.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
    KZGuideDrawing *drawing = [[KZGuideDrawing alloc] initWithFrame:NSMakeRect(0, 0, canvas.size.width, canvas.size.height)];
    drawing.destination = NSOffsetRect(destination, -canvas.origin.x, -canvas.origin.y);
    drawing.source = NSMakePoint(NSMidX(sourceRect) - canvas.origin.x, NSMidY(sourceRect) - canvas.origin.y);
    drawing.icon = [NSWorkspace.sharedWorkspace iconForFile:applicationURL().path];
    overlay.contentView = drawing;
    self.overlay = overlay;
    [overlay orderFrontRegardless];
    NSTimeInterval start = NSDate.timeIntervalSinceReferenceDate;
    NSRect companionFrame = self.window.frame;
    __weak KZPermissionGuide *weakSelf = self;
    self.timer = [NSTimer scheduledTimerWithTimeInterval:1.0/30 repeats:YES block:^(NSTimer *timer) {
        KZPermissionGuide *guide = weakSelf;
        NSTimeInterval elapsed = NSDate.timeIntervalSinceReferenceDate - start;
        NSString *front = NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier;
        BOOL relevant = [front isEqualToString:@"com.apple.systempreferences"] || [front isEqualToString:NSBundle.mainBundle.bundleIdentifier];
        if (!guide || !guide.window.visible || !relevant || elapsed > 4.8 ||
            !NSEqualRects(settingsFrame(), destination) || !NSEqualRects(guide.window.frame, companionFrame) ||
            NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion) {
            [timer invalidate]; [guide stop]; return;
        }
        drawing.progress = fmod(elapsed, 2.4) / 2.4;
        drawing.needsDisplay = YES;
    }];
}
@end

static char guideKey;
static KZPermissionGuide *guideFor(NSWindow *window) {
    KZPermissionGuide *guide = objc_getAssociatedObject(window, &guideKey);
    if (!guide) {
        guide = [KZPermissionGuide new];
        guide.window = window;
        objc_setAssociatedObject(window, &guideKey, guide, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    return guide;
}

bool kudzu_permission_drag_icon(void *pointer, double x, double y, double width, double height) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    KZPermissionGuide *guide = guideFor(window);
    if (!applicationURL()) return false;
    if (!guide.dragView) {
        guide.dragView = [KZAppDragView new];
        guide.dragView.guide = guide;
        guide.dragView.accessibilityElement = NO; // The web button supplies the keyboard alternative.
        [window.contentView addSubview:guide.dragView positioned:NSWindowAbove relativeTo:nil];
    }
    NSView *content = window.contentView;
    guide.dragView.frame = NSMakeRect(x, content.isFlipped ? y : content.bounds.size.height - y - height, width, height);
    return true;
}

bool kudzu_permission_open_settings(void *pointer, bool accessibility) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    KZPermissionGuide *guide = guideFor(window);
    [guide stop];
    NSString *pane = accessibility ? @"Privacy_Accessibility" : @"Privacy_ListenEvent";
    NSURL *url = [NSURL URLWithString:[@"x-apple.systempreferences:com.apple.preference.security?" stringByAppendingString:pane]];
    if (![NSWorkspace.sharedWorkspace openURL:url]) return false;
    NSUInteger generation = guide.generation;
    __weak KZPermissionGuide *weakGuide = guide;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 450 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
        [weakGuide showBesideSettings:0 generation:generation];
    });
    return true;
}

void kudzu_permission_stop_guide(void *pointer) {
    [guideFor((__bridge NSWindow *)pointer) stop];
}

bool kudzu_permission_reveal_app(void) {
    NSURL *url = applicationURL();
    if (!url) return false;
    [NSWorkspace.sharedWorkspace activateFileViewerSelectingURLs:@[url]];
    return true;
}

bool kudzu_permission_uses_device_control_label(void) {
    return NSProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 27;
}
