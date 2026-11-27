//! Native OS queries and tweaks: high contrast, reduced motion, dark title bar.

/// `(high_contrast, reduce_motion)` as reported by the OS, `None` where there is no native query.
pub(crate) fn native_flags() -> Option<(bool, bool)> {
    imp::flags()
}

/// System text scale factor (1.0 = none), clamped to 1.0..=3.0 for OS values.
pub(crate) fn system_text_scale() -> f32 {
    imp::text_scale().clamp(1.0, 3.0)
}

/// Asks the OS to draw the window title bar dark or light (Windows only; a no-op elsewhere).
pub(crate) fn set_dark_titlebar(window: &winit::window::Window, dark: bool) {
    imp::set_dark_titlebar(window, dark);
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub(super) fn set_dark_titlebar(window: &winit::window::Window, dark: bool) {
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::Win32(h) = handle.as_raw() else { return };
        let value: i32 = dark.into();
        // SAFETY: the HWND belongs to the live window we hold; the attribute value is a BOOL
        // passed by pointer with its exact size. A failure only leaves the title bar unchanged.
        unsafe {
            DwmSetWindowAttribute(
                h.hwnd.get() as _,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                (&value as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    pub(super) fn text_scale() -> f32 {
        use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let path: Vec<u16> = "Software\\Microsoft\\Accessibility\0".encode_utf16().collect();
        let name: Vec<u16> = "TextScaleFactor\0".encode_utf16().collect();
        let mut value: u32 = 100;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: path and name are NUL-terminated; the output buffer is a u32 of the size passed.
        // On failure (value absent) the default of 100 stays.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        if rc == 0 {
            value as f32 / 100.0
        } else {
            1.0
        }
    }

    use windows_sys::Win32::UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST},
    };

    pub(super) fn flags() -> Option<(bool, bool)> {
        // SAFETY: both calls write only into the local variables passed by pointer, with the
        // sizes the API expects.
        unsafe {
            let mut hc: HIGHCONTRASTW = std::mem::zeroed();
            hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
            let hc_ok = SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, (&mut hc as *mut HIGHCONTRASTW).cast(), 0) != 0;
            let mut anim: i32 = 1;
            let anim_ok = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut anim as *mut i32).cast(), 0) != 0;
            if !hc_ok && !anim_ok {
                return None;
            }
            Some((hc_ok && hc.dwFlags & HCF_HIGHCONTRASTON != 0, anim_ok && anim == 0))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub(super) fn set_dark_titlebar(_window: &winit::window::Window, _dark: bool) {}

    pub(super) fn flags() -> Option<(bool, bool)> {
        None
    }

    /// `QT_FONT_DPI` or GNOME's `text-scaling-factor`; 1.0 when neither is set.
    pub(super) fn text_scale() -> f32 {
        if let Some(dpi) = std::env::var("QT_FONT_DPI").ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|d| *d > 0.0) {
            return dpi / 96.0;
        }
        #[cfg(target_os = "linux")]
        {
            let out = std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "text-scaling-factor"]).output();
            if let Some(f) = out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<f32>().ok()) {
                return f;
            }
        }
        1.0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_query_matches_platform() {
        let flags = super::native_flags();
        assert_eq!(flags.is_some(), cfg!(windows));
    }
}
