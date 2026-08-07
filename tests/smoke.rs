//! Confirms the vendored libvpx actually links and exposes VP9 symbols.

use std::ffi::CStr;

#[test]
fn version_string_matches_vendored_libvpx() {
    let version = unsafe { CStr::from_ptr(vpx_sys::vpx_codec_version_str()) };
    assert!(version.to_str().unwrap().contains("1.16.0"));
}

#[test]
fn vp9_encoder_iface_is_available() {
    assert!(!unsafe { vpx_sys::vpx_codec_vp9_cx() }.is_null());
}
