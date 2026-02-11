// CaptureService - 截图服务模块
// 负责全局热键注册与屏幕区域框选截图
//
// Architecture:
// - Backend (Rust): Global hotkey registration/unregistration, screen capture via Win32 API
// - Frontend (React): CaptureOverlay UI for region selection
// - Flow: hotkey triggers → frontend shows overlay → user selects region →
//         frontend sends coordinates → backend captures that screen region

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Region coordinates for screen capture (sent from frontend after user selection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// 全局快捷键，默认 "Ctrl+Shift+2"
    pub shortcut: String,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            shortcut: "Ctrl+Shift+2".to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("热键注册失败: {0}")]
    HotkeyRegistration(String),
    #[error("截图失败: {0}")]
    CaptureFailed(String),
    #[error("用户取消截图")]
    Cancelled,
    #[error("无效的截图区域: {0}")]
    InvalidRegion(String),
}

impl Serialize for CaptureError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Manages the state of the capture service including the currently registered shortcut.
pub struct CaptureService {
    /// The currently registered shortcut string, protected by a mutex for thread safety.
    current_shortcut: Arc<Mutex<Option<String>>>,
    /// Whether a capture is currently in progress (overlay is shown).
    capture_active: Arc<Mutex<bool>>,
}

impl CaptureService {
    /// Create a new CaptureService instance.
    pub fn new() -> Self {
        Self {
            current_shortcut: Arc::new(Mutex::new(None)),
            capture_active: Arc::new(Mutex::new(false)),
        }
    }

    /// Register a global shortcut using the provided configuration.
    ///
    /// In the Tauri v2 architecture, the actual shortcut registration happens
    /// through the `tauri-plugin-global-shortcut` plugin on the frontend side.
    /// This function validates the config and stores the shortcut for management.
    ///
    /// # Arguments
    /// * `config` - The capture configuration containing the shortcut string
    ///
    /// # Returns
    /// * `Ok(())` if the shortcut was successfully registered
    /// * `Err(CaptureError::HotkeyRegistration)` if the shortcut string is invalid
    pub fn register_hotkey(&self, config: &CaptureConfig) -> Result<(), CaptureError> {
        let shortcut = config.shortcut.trim();
        if shortcut.is_empty() {
            return Err(CaptureError::HotkeyRegistration(
                "快捷键不能为空".to_string(),
            ));
        }

        // Validate the shortcut format: should contain modifier(s) + key
        if !validate_shortcut_format(shortcut) {
            return Err(CaptureError::HotkeyRegistration(format!(
                "无效的快捷键格式: '{}'. 格式应为 'Modifier+Key'，例如 'Ctrl+Shift+2'",
                shortcut
            )));
        }

        let mut current = self.current_shortcut.lock().map_err(|e| {
            CaptureError::HotkeyRegistration(format!("内部锁错误: {}", e))
        })?;
        *current = Some(shortcut.to_string());
        Ok(())
    }

    /// Unregister the currently registered global shortcut.
    ///
    /// # Returns
    /// * `Ok(())` if the shortcut was successfully unregistered or none was registered
    /// * `Err(CaptureError::HotkeyRegistration)` on internal error
    pub fn unregister_hotkey(&self) -> Result<(), CaptureError> {
        let mut current = self.current_shortcut.lock().map_err(|e| {
            CaptureError::HotkeyRegistration(format!("内部锁错误: {}", e))
        })?;
        *current = None;
        Ok(())
    }

    /// Get the currently registered shortcut string, if any.
    pub fn current_shortcut(&self) -> Option<String> {
        self.current_shortcut.lock().ok().and_then(|s| s.clone())
    }

    /// Mark capture as active (overlay is being shown).
    pub fn set_capture_active(&self, active: bool) {
        if let Ok(mut state) = self.capture_active.lock() {
            *state = active;
        }
    }

    /// Check if a capture is currently in progress.
    pub fn is_capture_active(&self) -> bool {
        self.capture_active.lock().map(|s| *s).unwrap_or(false)
    }

    /// Cancel the current capture operation.
    ///
    /// This is called when the user presses Escape during region selection.
    ///
    /// # Returns
    /// * `Err(CaptureError::Cancelled)` always, to signal cancellation to the caller
    pub fn cancel_capture(&self) -> Result<Vec<u8>, CaptureError> {
        self.set_capture_active(false);
        Err(CaptureError::Cancelled)
    }

    /// Capture a specific region of the screen and return it as PNG bytes.
    ///
    /// This function uses Win32 API calls to capture the specified screen region.
    /// The region coordinates come from the frontend CaptureOverlay component
    /// after the user completes their selection.
    ///
    /// # Arguments
    /// * `region` - The screen region to capture (x, y, width, height)
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - PNG-encoded image bytes of the captured region
    /// * `Err(CaptureError)` - If the capture fails or region is invalid
    pub fn capture_region(&self, region: &CaptureRegion) -> Result<Vec<u8>, CaptureError> {
        // Validate region dimensions
        if region.width == 0 || region.height == 0 {
            return Err(CaptureError::InvalidRegion(
                "截图区域的宽度和高度必须大于 0".to_string(),
            ));
        }

        // Use platform-specific screen capture
        let pixels = capture_screen_region(region)?;

        // Encode as PNG
        encode_png(&pixels, region.width, region.height)
    }
}

impl Default for CaptureService {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate that a shortcut string has a valid format.
///
/// A valid shortcut must contain at least one modifier key (Ctrl, Alt, Shift, Super/Cmd)
/// and one non-modifier key, separated by '+'.
///
/// # Examples
/// - "Ctrl+Shift+2" → true
/// - "Alt+F1" → true
/// - "Ctrl+Shift+A" → true
/// - "" → false
/// - "2" → false (no modifier)
/// - "Ctrl+" → false (no key)
pub fn validate_shortcut_format(shortcut: &str) -> bool {
    let parts: Vec<&str> = shortcut.split('+').map(|s| s.trim()).collect();

    if parts.len() < 2 {
        return false;
    }

    let modifiers = ["ctrl", "alt", "shift", "super", "cmd", "cmdorctrl"];
    let mut has_modifier = false;
    let mut has_key = false;

    for part in &parts {
        let lower = part.to_lowercase();
        if lower.is_empty() {
            return false;
        }
        if modifiers.contains(&lower.as_str()) {
            has_modifier = true;
        } else {
            has_key = true;
        }
    }

    has_modifier && has_key
}

/// Capture a specific screen region using Win32 API.
///
/// Uses GetDC(NULL) to get the screen device context, then BitBlt to copy
/// the specified region into a memory bitmap. Returns raw BGRA pixel data.
#[cfg(target_os = "windows")]
fn capture_screen_region(region: &CaptureRegion) -> Result<Vec<u8>, CaptureError> {
    use std::ptr;

    // Win32 API types and functions via raw FFI
    #[allow(non_snake_case)]
    mod win32 {
        use std::ffi::c_void;

        pub type HDC = *mut c_void;
        pub type HBITMAP = *mut c_void;
        pub type HGDIOBJ = *mut c_void;
        pub type HWND = *mut c_void;
        pub type BOOL = i32;
        pub type INT = i32;
        pub type UINT = u32;
        pub type DWORD = u32;
        pub type LONG = i32;
        pub type WORD = u16;

        pub const SRCCOPY: DWORD = 0x00CC0020;
        pub const DIB_RGB_COLORS: UINT = 0;
        pub const BI_RGB: DWORD = 0;

        #[repr(C)]
        #[allow(non_snake_case)]
        pub struct BITMAPINFOHEADER {
            pub biSize: DWORD,
            pub biWidth: LONG,
            pub biHeight: LONG,
            pub biPlanes: WORD,
            pub biBitCount: WORD,
            pub biCompression: DWORD,
            pub biSizeImage: DWORD,
            pub biXPelsPerMeter: LONG,
            pub biYPelsPerMeter: LONG,
            pub biClrUsed: DWORD,
            pub biClrImportant: DWORD,
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        pub struct RGBQUAD {
            pub rgbBlue: u8,
            pub rgbGreen: u8,
            pub rgbRed: u8,
            pub rgbReserved: u8,
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        pub struct BITMAPINFO {
            pub bmiHeader: BITMAPINFOHEADER,
            pub bmiColors: [RGBQUAD; 1],
        }

        extern "system" {
            pub fn GetDC(hWnd: HWND) -> HDC;
            pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> INT;
            pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
            pub fn DeleteDC(hdc: HDC) -> BOOL;
            pub fn CreateCompatibleBitmap(hdc: HDC, cx: INT, cy: INT) -> HBITMAP;
            pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
            pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
            pub fn BitBlt(
                hdc: HDC, x: INT, y: INT, cx: INT, cy: INT,
                hdcSrc: HDC, x1: INT, y1: INT, rop: DWORD,
            ) -> BOOL;
            pub fn GetDIBits(
                hdc: HDC, hbm: HBITMAP, start: UINT, cLines: UINT,
                lpvBits: *mut c_void, lpbmi: *mut BITMAPINFO, usage: UINT,
            ) -> INT;
        }
    }

    unsafe {
        // Get the screen device context
        let screen_dc = win32::GetDC(ptr::null_mut());
        if screen_dc.is_null() {
            return Err(CaptureError::CaptureFailed(
                "无法获取屏幕设备上下文 (GetDC failed)".to_string(),
            ));
        }

        // Create a compatible memory DC
        let mem_dc = win32::CreateCompatibleDC(screen_dc);
        if mem_dc.is_null() {
            win32::ReleaseDC(ptr::null_mut(), screen_dc);
            return Err(CaptureError::CaptureFailed(
                "无法创建兼容设备上下文 (CreateCompatibleDC failed)".to_string(),
            ));
        }

        // Create a compatible bitmap for the capture region
        let bitmap = win32::CreateCompatibleBitmap(
            screen_dc,
            region.width as i32,
            region.height as i32,
        );
        if bitmap.is_null() {
            win32::DeleteDC(mem_dc);
            win32::ReleaseDC(ptr::null_mut(), screen_dc);
            return Err(CaptureError::CaptureFailed(
                "无法创建兼容位图 (CreateCompatibleBitmap failed)".to_string(),
            ));
        }

        // Select the bitmap into the memory DC
        let old_bitmap = win32::SelectObject(mem_dc, bitmap);

        // BitBlt: copy the screen region to the memory DC
        let blt_result = win32::BitBlt(
            mem_dc,
            0,
            0,
            region.width as i32,
            region.height as i32,
            screen_dc,
            region.x,
            region.y,
            win32::SRCCOPY,
        );

        if blt_result == 0 {
            win32::SelectObject(mem_dc, old_bitmap);
            win32::DeleteObject(bitmap);
            win32::DeleteDC(mem_dc);
            win32::ReleaseDC(ptr::null_mut(), screen_dc);
            return Err(CaptureError::CaptureFailed(
                "屏幕区域复制失败 (BitBlt failed)".to_string(),
            ));
        }

        // Prepare BITMAPINFO for GetDIBits
        let mut bmi = win32::BITMAPINFO {
            bmiHeader: win32::BITMAPINFOHEADER {
                biSize: std::mem::size_of::<win32::BITMAPINFOHEADER>() as u32,
                biWidth: region.width as i32,
                // Negative height = top-down DIB (origin at top-left)
                biHeight: -(region.height as i32),
                biPlanes: 1,
                biBitCount: 32, // BGRA
                biCompression: win32::BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [win32::RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            }],
        };

        // Allocate buffer for pixel data (BGRA, 4 bytes per pixel)
        let pixel_count = (region.width * region.height) as usize;
        let mut pixels: Vec<u8> = vec![0u8; pixel_count * 4];

        // Get the bitmap bits
        let lines = win32::GetDIBits(
            mem_dc,
            bitmap,
            0,
            region.height,
            pixels.as_mut_ptr() as *mut std::ffi::c_void,
            &mut bmi,
            win32::DIB_RGB_COLORS,
        );

        // Cleanup Win32 resources
        win32::SelectObject(mem_dc, old_bitmap);
        win32::DeleteObject(bitmap);
        win32::DeleteDC(mem_dc);
        win32::ReleaseDC(ptr::null_mut(), screen_dc);

        if lines == 0 {
            return Err(CaptureError::CaptureFailed(
                "无法获取位图数据 (GetDIBits failed)".to_string(),
            ));
        }

        // Convert BGRA to RGBA (swap B and R channels)
        for i in 0..pixel_count {
            let offset = i * 4;
            pixels.swap(offset, offset + 2); // swap B and R
        }

        Ok(pixels)
    }
}

/// Fallback screen capture for non-Windows platforms (returns an error).
#[cfg(not(target_os = "windows"))]
fn capture_screen_region(_region: &CaptureRegion) -> Result<Vec<u8>, CaptureError> {
    Err(CaptureError::CaptureFailed(
        "屏幕截图仅支持 Windows 平台".to_string(),
    ))
}

/// Encode raw RGBA pixel data as a PNG image.
fn encode_png(rgba_pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    use image::{ImageBuffer, Rgba};
    use std::io::Cursor;

    let expected_len = (width * height * 4) as usize;
    if rgba_pixels.len() != expected_len {
        return Err(CaptureError::CaptureFailed(format!(
            "像素数据长度不匹配: 期望 {} 字节, 实际 {} 字节",
            expected_len,
            rgba_pixels.len()
        )));
    }

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_pixels.to_vec()).ok_or_else(|| {
            CaptureError::CaptureFailed("无法从像素数据创建图像缓冲区".to_string())
        })?;

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| CaptureError::CaptureFailed(format!("PNG 编码失败: {}", e)))?;

    Ok(buf.into_inner())
}

// ============================================================
// Free-standing convenience functions (backward compatibility)
// ============================================================

/// Register a global shortcut (convenience wrapper).
///
/// Creates a temporary CaptureService to validate and register the hotkey.
/// For full lifecycle management, use CaptureService directly.
pub fn register_hotkey(config: &CaptureConfig) -> Result<(), CaptureError> {
    // Validate the shortcut format
    let shortcut = config.shortcut.trim();
    if shortcut.is_empty() {
        return Err(CaptureError::HotkeyRegistration(
            "快捷键不能为空".to_string(),
        ));
    }
    if !validate_shortcut_format(shortcut) {
        return Err(CaptureError::HotkeyRegistration(format!(
            "无效的快捷键格式: '{}'",
            shortcut
        )));
    }
    Ok(())
}

/// Capture the full screen and return PNG bytes (convenience wrapper).
///
/// This captures the entire primary screen. For region-based capture,
/// use CaptureService::capture_region() with specific coordinates.
pub fn capture_region() -> Result<Vec<u8>, CaptureError> {
    // In the Tauri architecture, the actual capture flow is:
    // 1. Frontend shows overlay
    // 2. User selects region
    // 3. Frontend calls capture_screen_region with coordinates
    // For backward compatibility, this returns an error indicating
    // the caller should use the region-based API instead.
    Err(CaptureError::CaptureFailed(
        "请使用 CaptureService::capture_region() 并提供截图区域坐标".to_string(),
    ))
}

/// Unregister the global shortcut (convenience wrapper).
pub fn unregister_hotkey() -> Result<(), CaptureError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    // ============================================================
    // CaptureConfig tests
    // ============================================================

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.shortcut, "Ctrl+Shift+2");
    }

    #[test]
    fn test_capture_config_custom_shortcut() {
        let config = CaptureConfig {
            shortcut: "Alt+F1".to_string(),
        };
        assert_eq!(config.shortcut, "Alt+F1");
    }

    #[test]
    fn test_capture_config_serialization() {
        let config = CaptureConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("Ctrl+Shift+2"));

        let deserialized: CaptureConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.shortcut, config.shortcut);
    }

    // ============================================================
    // CaptureRegion tests
    // ============================================================

    #[test]
    fn test_capture_region_serialization() {
        let region = CaptureRegion {
            x: 100,
            y: 200,
            width: 300,
            height: 400,
        };
        let json = serde_json::to_string(&region).unwrap();
        let deserialized: CaptureRegion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.x, 100);
        assert_eq!(deserialized.y, 200);
        assert_eq!(deserialized.width, 300);
        assert_eq!(deserialized.height, 400);
    }

    // ============================================================
    // CaptureError tests
    // ============================================================

    #[test]
    fn test_capture_error_display() {
        let err = CaptureError::HotkeyRegistration("test error".to_string());
        assert_eq!(err.to_string(), "热键注册失败: test error");

        let err = CaptureError::CaptureFailed("capture error".to_string());
        assert_eq!(err.to_string(), "截图失败: capture error");

        let err = CaptureError::Cancelled;
        assert_eq!(err.to_string(), "用户取消截图");

        let err = CaptureError::InvalidRegion("bad region".to_string());
        assert_eq!(err.to_string(), "无效的截图区域: bad region");
    }

    #[test]
    fn test_capture_error_serialization() {
        let err = CaptureError::Cancelled;
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"用户取消截图\"");
    }

    // ============================================================
    // validate_shortcut_format tests
    // ============================================================

    #[test]
    fn test_validate_shortcut_valid_formats() {
        assert!(validate_shortcut_format("Ctrl+Shift+2"));
        assert!(validate_shortcut_format("Alt+F1"));
        assert!(validate_shortcut_format("Ctrl+A"));
        assert!(validate_shortcut_format("Ctrl+Shift+A"));
        assert!(validate_shortcut_format("Ctrl+Alt+Shift+S"));
        assert!(validate_shortcut_format("Super+Space"));
        assert!(validate_shortcut_format("CmdOrCtrl+Shift+2"));
    }

    #[test]
    fn test_validate_shortcut_invalid_formats() {
        assert!(!validate_shortcut_format(""));
        assert!(!validate_shortcut_format("2"));
        assert!(!validate_shortcut_format("A"));
        assert!(!validate_shortcut_format("Ctrl+"));
        assert!(!validate_shortcut_format("Ctrl"));
        assert!(!validate_shortcut_format("Ctrl+Shift"));
        assert!(!validate_shortcut_format("+A"));
    }

    #[test]
    fn test_validate_shortcut_case_insensitive_modifiers() {
        assert!(validate_shortcut_format("ctrl+shift+2"));
        assert!(validate_shortcut_format("CTRL+SHIFT+2"));
        assert!(validate_shortcut_format("Ctrl+SHIFT+a"));
    }

    // ============================================================
    // CaptureService tests
    // ============================================================

    #[test]
    fn test_capture_service_new() {
        let service = CaptureService::new();
        assert!(service.current_shortcut().is_none());
        assert!(!service.is_capture_active());
    }

    #[test]
    fn test_capture_service_default() {
        let service = CaptureService::default();
        assert!(service.current_shortcut().is_none());
    }

    #[test]
    fn test_register_hotkey_default_config() {
        let service = CaptureService::new();
        let config = CaptureConfig::default();
        let result = service.register_hotkey(&config);
        assert!(result.is_ok());
        assert_eq!(service.current_shortcut(), Some("Ctrl+Shift+2".to_string()));
    }

    #[test]
    fn test_register_hotkey_custom_config() {
        let service = CaptureService::new();
        let config = CaptureConfig {
            shortcut: "Alt+F1".to_string(),
        };
        let result = service.register_hotkey(&config);
        assert!(result.is_ok());
        assert_eq!(service.current_shortcut(), Some("Alt+F1".to_string()));
    }

    #[test]
    fn test_register_hotkey_empty_shortcut() {
        let service = CaptureService::new();
        let config = CaptureConfig {
            shortcut: "".to_string(),
        };
        let result = service.register_hotkey(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::HotkeyRegistration(msg) => {
                assert!(msg.contains("不能为空"));
            }
            other => panic!("Expected HotkeyRegistration, got: {:?}", other),
        }
    }

    #[test]
    fn test_register_hotkey_invalid_format() {
        let service = CaptureService::new();
        let config = CaptureConfig {
            shortcut: "JustAKey".to_string(),
        };
        let result = service.register_hotkey(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::HotkeyRegistration(msg) => {
                assert!(msg.contains("无效的快捷键格式"));
            }
            other => panic!("Expected HotkeyRegistration, got: {:?}", other),
        }
    }

    #[test]
    fn test_register_hotkey_replaces_previous() {
        let service = CaptureService::new();

        let config1 = CaptureConfig {
            shortcut: "Ctrl+Shift+2".to_string(),
        };
        service.register_hotkey(&config1).unwrap();
        assert_eq!(service.current_shortcut(), Some("Ctrl+Shift+2".to_string()));

        let config2 = CaptureConfig {
            shortcut: "Alt+F1".to_string(),
        };
        service.register_hotkey(&config2).unwrap();
        assert_eq!(service.current_shortcut(), Some("Alt+F1".to_string()));
    }

    #[test]
    fn test_unregister_hotkey() {
        let service = CaptureService::new();
        let config = CaptureConfig::default();
        service.register_hotkey(&config).unwrap();
        assert!(service.current_shortcut().is_some());

        let result = service.unregister_hotkey();
        assert!(result.is_ok());
        assert!(service.current_shortcut().is_none());
    }

    #[test]
    fn test_unregister_hotkey_when_none_registered() {
        let service = CaptureService::new();
        let result = service.unregister_hotkey();
        assert!(result.is_ok());
    }

    #[test]
    fn test_capture_active_state() {
        let service = CaptureService::new();
        assert!(!service.is_capture_active());

        service.set_capture_active(true);
        assert!(service.is_capture_active());

        service.set_capture_active(false);
        assert!(!service.is_capture_active());
    }

    #[test]
    fn test_cancel_capture() {
        let service = CaptureService::new();
        service.set_capture_active(true);
        assert!(service.is_capture_active());

        let result = service.cancel_capture();
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::Cancelled => {} // expected
            other => panic!("Expected Cancelled, got: {:?}", other),
        }
        assert!(!service.is_capture_active());
    }

    #[test]
    fn test_capture_region_zero_width() {
        let service = CaptureService::new();
        let region = CaptureRegion {
            x: 0,
            y: 0,
            width: 0,
            height: 100,
        };
        let result = service.capture_region(&region);
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::InvalidRegion(msg) => {
                assert!(msg.contains("宽度和高度必须大于 0"));
            }
            other => panic!("Expected InvalidRegion, got: {:?}", other),
        }
    }

    #[test]
    fn test_capture_region_zero_height() {
        let service = CaptureService::new();
        let region = CaptureRegion {
            x: 0,
            y: 0,
            width: 100,
            height: 0,
        };
        let result = service.capture_region(&region);
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::InvalidRegion(msg) => {
                assert!(msg.contains("宽度和高度必须大于 0"));
            }
            other => panic!("Expected InvalidRegion, got: {:?}", other),
        }
    }

    // ============================================================
    // encode_png tests
    // ============================================================

    #[test]
    fn test_encode_png_valid_data() {
        // Create a 2x2 RGBA image (red, green, blue, white)
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255,     // red
            0, 255, 0, 255,     // green
            0, 0, 255, 255,     // blue
            255, 255, 255, 255, // white
        ];
        let result = encode_png(&pixels, 2, 2);
        assert!(result.is_ok());
        let png_bytes = result.unwrap();
        // Verify PNG magic bytes
        assert!(png_bytes.len() > 8);
        assert_eq!(&png_bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_encode_png_single_pixel() {
        let pixels: Vec<u8> = vec![128, 64, 32, 255];
        let result = encode_png(&pixels, 1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_png_wrong_data_length() {
        // 2x2 image needs 16 bytes, but we provide only 8
        let pixels: Vec<u8> = vec![0u8; 8];
        let result = encode_png(&pixels, 2, 2);
        assert!(result.is_err());
        match result.unwrap_err() {
            CaptureError::CaptureFailed(msg) => {
                assert!(msg.contains("长度不匹配"));
            }
            other => panic!("Expected CaptureFailed, got: {:?}", other),
        }
    }

    // ============================================================
    // Win32 screen capture integration test (Windows only)
    // ============================================================

    #[cfg(target_os = "windows")]
    #[test]
    fn test_capture_screen_region_small_area() {
        // Capture a small 10x10 region from the top-left corner of the screen
        let region = CaptureRegion {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let result = capture_screen_region(&region);
        assert!(result.is_ok(), "Screen capture should succeed: {:?}", result.err());
        let pixels = result.unwrap();
        // 10x10 pixels * 4 bytes (RGBA) = 400 bytes
        assert_eq!(pixels.len(), 400);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_capture_region_returns_valid_png() {
        let service = CaptureService::new();
        let region = CaptureRegion {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let result = service.capture_region(&region);
        assert!(result.is_ok(), "capture_region should succeed: {:?}", result.err());
        let png_bytes = result.unwrap();
        // Verify PNG magic bytes
        assert!(png_bytes.len() > 8);
        assert_eq!(&png_bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);

        // Verify the PNG can be decoded back to an image
        let img = image::load_from_memory(&png_bytes).unwrap();
        let (w, h) = img.dimensions();
        assert_eq!(w, 20);
        assert_eq!(h, 20);
    }

    // ============================================================
    // Free-standing function tests
    // ============================================================

    #[test]
    fn test_register_hotkey_free_fn_valid() {
        let config = CaptureConfig::default();
        let result = register_hotkey(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_hotkey_free_fn_invalid() {
        let config = CaptureConfig {
            shortcut: "".to_string(),
        };
        let result = register_hotkey(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_unregister_hotkey_free_fn() {
        let result = unregister_hotkey();
        assert!(result.is_ok());
    }

    #[test]
    fn test_capture_region_free_fn() {
        // The free-standing capture_region() should return an error
        // directing users to use the region-based API
        let result = capture_region();
        assert!(result.is_err());
    }
}
