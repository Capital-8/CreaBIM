//! Whether this computer has a touchpad (issue #78).
//!
//! Windows reports a touchpad's two-finger scroll and a mouse wheel the
//! same way, so a view cannot tell them apart from the scroll alone. What
//! it can know is whether a touchpad is there at all: asked of the system
//! once, from its list of input devices. Where it cannot be asked, one is
//! taken to be there, and the views tell by the scroll as before.

/// Is a touchpad plugged in or built in? Asked once.
pub fn present() -> bool {
    static PRESENT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PRESENT.get_or_init(|| {
        let found = find();
        crate::actlog::line(match found {
            Some(true) => "wheel: this computer has a touchpad (two fingers pan)",
            Some(false) => "wheel: no touchpad here, so a wheel always zooms",
            None => "wheel: couldn't ask for a touchpad, telling by the scroll",
        });
        found.unwrap_or(true)
    })
}

#[cfg(not(windows))]
fn find() -> Option<bool> {
    None
}

/// From the raw input device list: a HID device on the Digitizers page
/// (0x0D) with the Touch Pad usage (0x05).
#[cfg(windows)]
fn find() -> Option<bool> {
    use core::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RawInputDeviceList {
        device: *mut c_void,
        kind: u32,
    }
    // RID_DEVICE_INFO: its size, its type, then the largest member (the
    // keyboard's six u32); a HID's ids, version, usage page and usage come
    // first in that space
    #[repr(C)]
    struct RidDeviceInfo {
        size: u32,
        kind: u32,
        vendor: u32,
        product: u32,
        version: u32,
        usage_page: u16,
        usage: u16,
        _rest: [u32; 2],
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetRawInputDeviceList(list: *mut RawInputDeviceList, count: *mut u32, size: u32) -> u32;
        fn GetRawInputDeviceInfoW(
            device: *mut c_void,
            command: u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> u32;
    }
    const RIM_TYPEHID: u32 = 2;
    const RIDI_DEVICEINFO: u32 = 0x2000_000b;
    let each = core::mem::size_of::<RawInputDeviceList>() as u32;
    let mut count = 0u32;
    // SAFETY: a null list asks only for the count, written to `count`.
    if unsafe { GetRawInputDeviceList(core::ptr::null_mut(), &mut count, each) } != 0 {
        return None;
    }
    if count == 0 {
        return Some(false);
    }
    let mut list = vec![
        RawInputDeviceList {
            device: core::ptr::null_mut(),
            kind: 0
        };
        count as usize
    ];
    // SAFETY: `list` holds `count` entries of the size passed.
    let got = unsafe { GetRawInputDeviceList(list.as_mut_ptr(), &mut count, each) };
    if got == u32::MAX {
        return None;
    }
    for d in list
        .iter()
        .take(got as usize)
        .filter(|d| d.kind == RIM_TYPEHID)
    {
        let mut info = RidDeviceInfo {
            size: core::mem::size_of::<RidDeviceInfo>() as u32,
            kind: 0,
            vendor: 0,
            product: 0,
            version: 0,
            usage_page: 0,
            usage: 0,
            _rest: [0; 2],
        };
        let mut size = info.size;
        // SAFETY: `info` is `size` bytes, and says so in its first field.
        let n = unsafe {
            GetRawInputDeviceInfoW(
                d.device,
                RIDI_DEVICEINFO,
                (&mut info as *mut RidDeviceInfo).cast(),
                &mut size,
            )
        };
        if n != u32::MAX && n != 0 && info.usage_page == 0x0D && info.usage == 0x05 {
            return Some(true);
        }
    }
    Some(false)
}
