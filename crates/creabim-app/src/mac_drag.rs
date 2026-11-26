//! The AppKit hit region beside the standard window buttons.

use objc2::rc::Retained;
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSEvent, NSUserInterfaceLayoutDirection, NSView, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const BUTTONS_WIDTH: f64 = 82.0;
const DRAG_WIDTH: f64 = 48.0;
pub(crate) const TITLEBAR_HEIGHT: f32 = 36.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Layout {
    pub right: bool,
    pub zoom: f32,
}

impl Layout {
    pub fn inset(self) -> f64 {
        (BUTTONS_WIDTH + DRAG_WIDTH) / f64::from(self.zoom)
    }
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    struct DragView;

    impl DragView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(window) = self.window() {
                let app = NSApplication::sharedApplication(MainThreadMarker::from(self));
                if window.isMovable() && window.attachedSheet().is_none() && app.modalWindow().is_none() {
                    window.performWindowDragWithEvent(event);
                }
            }
        }
    }
);

pub(crate) struct MacDrag {
    view: Retained<DragView>,
}

impl MacDrag {
    pub fn new(handle: &impl HasWindowHandle) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let RawWindowHandle::AppKit(handle) = handle.window_handle().ok()?.as_raw() else {
            return None;
        };
        // The live AppKit handle borrows the Winit content view. AppKit operations
        // here run exclusively on the main thread.
        let content = unsafe { &*handle.ns_view.cast::<NSView>().as_ptr() };
        let view = mtm.alloc::<DragView>().set_ivars(());
        let view: Retained<DragView> =
            unsafe { msg_send![super(view), initWithFrame: NSRect::ZERO] };
        view.setHidden(true);
        content.addSubview(&view);
        Some(Self { view })
    }

    pub fn layout(&self, zoom: f32) -> Option<Layout> {
        Some(Layout {
            right: self.view.window()?.windowTitlebarLayoutDirection()
                == NSUserInterfaceLayoutDirection::RightToLeft,
            zoom,
        })
    }

    pub fn place(&self, layout: Layout, enabled: bool) {
        let (Some(window), Some(content)) = (self.view.window(), unsafe { self.view.superview() })
        else {
            return;
        };
        let bounds = content.bounds();
        let height = f64::from(TITLEBAR_HEIGHT * layout.zoom);
        let inset = BUTTONS_WIDTH + DRAG_WIDTH;
        let visible = enabled
            && !window.styleMask().contains(NSWindowStyleMask::FullScreen)
            && bounds.size.width >= inset
            && bounds.size.height >= height;
        self.view.setHidden(!visible);
        if visible {
            let x = if layout.right {
                bounds.size.width - inset
            } else {
                BUTTONS_WIDTH
            };
            let y = if content.isFlipped() {
                0.0
            } else {
                bounds.size.height - height
            };
            let frame = NSRect::new(
                NSPoint::new(bounds.origin.x + x, bounds.origin.y + y),
                NSSize::new(DRAG_WIDTH, height),
            );
            if self.view.frame() != frame {
                self.view.setFrame(frame);
            }
        }
    }
}

impl Drop for MacDrag {
    fn drop(&mut self) {
        self.view.removeFromSuperview();
    }
}
