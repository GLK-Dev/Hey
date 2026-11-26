//! Native accessibility queries: high contrast and reduced motion.

/// `(high_contrast, reduce_motion)` as reported by the OS, `None` where there is no native query.
pub(crate) fn native_flags() -> Option<(bool, bool)> {
    imp::flags()
}

#[cfg(windows)]
mod imp {
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
    pub(super) fn flags() -> Option<(bool, bool)> {
        None
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
