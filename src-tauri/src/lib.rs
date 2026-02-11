// FormulaSnap - 离线桌面端公式截图识别工具
// Rust 后端库入口

pub mod capture;
pub mod clipboard;
pub mod convert;
pub mod export;
pub mod history;
pub mod ocr;
pub mod preprocess;

use capture::CaptureRegion;
use history::HistoryRecord;
use ocr::OcrResult;
use export::TexExportOptions;
use tauri::Manager;

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
async fn capture_screenshot() -> Result<Vec<u8>, String> {
    capture::capture_region().map_err(|e| e.to_string())
}

/// Capture a specific screen region and return PNG bytes.
/// Called by the frontend after the user selects a region in the CaptureOverlay.
#[tauri::command]
async fn capture_screen_region(region: CaptureRegion) -> Result<Vec<u8>, String> {
    let service = capture::CaptureService::new();
    service.capture_region(&region).map_err(|e| e.to_string())
}

/// Cancel the current capture operation (called when user presses Escape).
#[tauri::command]
async fn cancel_capture() -> Result<(), String> {
    // Return a cancellation signal to the frontend
    Err("用户取消截图".to_string())
}

/// 使用 texify 进行公式识别
/// 
/// 优先使用打包的 ocr_engine.exe（PyInstaller 打包），
/// 回退到 Python 脚本调用。
#[tauri::command]
async fn recognize_formula(image: Vec<u8>, app_handle: tauri::AppHandle) -> Result<OcrResult, String> {
    use std::process::Command;
    use std::io::Write;

    // 将图片写入临时文件
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("formulasnap_ocr_input.png");
    
    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("无法创建临时文件: {}", e))?;
        file.write_all(&image)
            .map_err(|e| format!("无法写入临时文件: {}", e))?;
    }

    // 获取 OCR 引擎路径
    let (ocr_cmd, ocr_args) = get_ocr_command(&app_handle, &temp_path)?;

    // 调用 OCR 引擎
    let output = Command::new(&ocr_cmd)
        .args(&ocr_args)
        .output()
        .map_err(|e| format!("无法启动 OCR 引擎: {}", e))?;

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("OCR 识别失败: {}", stderr));
    }

    // 解析 JSON 输出
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("解析 OCR 结果失败: {}。输出: {}", e, stdout))?;

    if let Some(error) = result.get("error") {
        return Err(format!("OCR 错误: {}", error));
    }

    let latex = result.get("latex")
        .and_then(|v| v.as_str())
        .ok_or("OCR 结果缺少 latex 字段")?
        .to_string();

    let confidence = result.get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.9);

    Ok(OcrResult { latex, confidence })
}

/// 获取 OCR 命令和参数
/// 优先使用打包的 ocr_engine.exe，回退到 Python 脚本
fn get_ocr_command(app_handle: &tauri::AppHandle, image_path: &std::path::Path) -> Result<(String, Vec<String>), String> {
    use tauri::Manager;
    
    let image_arg = image_path.to_string_lossy().to_string();
    let mut searched_paths: Vec<String> = Vec::new();
    
    // 1. 首先尝试打包的 ocr_engine.exe（生产环境）
    if let Ok(resource_path) = app_handle.path().resource_dir() {
        // Windows: ocr_engine/ocr_engine.exe
        let exe_path = resource_path.join("ocr_engine").join("ocr_engine.exe");
        searched_paths.push(exe_path.to_string_lossy().to_string());
        if exe_path.exists() {
            return Ok((exe_path.to_string_lossy().to_string(), vec![image_arg]));
        }
        
        // 直接在资源目录下
        let exe_direct = resource_path.join("ocr_engine.exe");
        searched_paths.push(exe_direct.to_string_lossy().to_string());
        if exe_direct.exists() {
            return Ok((exe_direct.to_string_lossy().to_string(), vec![image_arg]));
        }
    }
    
    // 2. 开发模式：尝试本地打包的 ocr_engine
    let dev_exe_paths = [
        "ocr_engine/ocr_engine.exe",
        "../src-tauri/ocr_engine/ocr_engine.exe",
    ];
    
    for path in &dev_exe_paths {
        searched_paths.push(path.to_string());
        if std::path::Path::new(path).exists() {
            return Ok((path.to_string(), vec![image_arg]));
        }
    }
    
    // 3. 回退到 Python 脚本（开发模式）
    let script_paths = [
        "../scripts/ocr_server.py",
        "scripts/ocr_server.py",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/ocr_server.py"),
    ];
    
    for path in &script_paths {
        searched_paths.push(path.to_string());
        if std::path::Path::new(path).exists() {
            let python = get_python_path();
            let script = std::path::Path::new(path)
                .canonicalize()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string());
            return Ok((python, vec![script, image_arg]));
        }
    }

    Err(format!("OCR 引擎不存在，请重新安装应用。\n\n已搜索路径:\n{}", searched_paths.join("\n")))
}

/// 获取 Python 解释器路径
fn get_python_path() -> String {
    // 优先使用 Texify 专用虚拟环境
    let texify_venv_paths = [
        "../.venv-texify/Scripts/python.exe",  // Windows venv
        "../.venv-texify/bin/python",          // Unix venv
        concat!(env!("CARGO_MANIFEST_DIR"), "/../.venv-texify/Scripts/python.exe"),
    ];

    for path in &texify_venv_paths {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    // 回退到主虚拟环境
    let venv_paths = [
        "../.venv/Scripts/python.exe",
        "../.venv/bin/python",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../.venv/Scripts/python.exe"),
    ];

    for path in &venv_paths {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    // 回退到系统 Python
    "python".to_string()
}

#[tauri::command]
async fn convert_to_omml(latex: String) -> Result<String, String> {
    eprintln!("[convert_to_omml] Input LaTeX length: {}", latex.len());
    match convert::latex_to_omml(&latex) {
        Ok(omml) => {
            eprintln!("[convert_to_omml] Success! OMML length: {}", omml.len());
            Ok(omml)
        }
        Err(e) => {
            eprintln!("[convert_to_omml] FAILED: {:?}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn convert_to_mathml(latex: String) -> Result<String, String> {
    eprintln!("[convert_to_mathml] Input LaTeX: {}", latex);
    match convert::latex_to_mathml(&latex) {
        Ok(mathml) => {
            eprintln!("[convert_to_mathml] Success! MathML length: {}", mathml.len());
            Ok(mathml)
        }
        Err(e) => {
            eprintln!("[convert_to_mathml] FAILED: {:?}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn copy_formula_to_clipboard(
    latex: String,
    omml: String,
    mathml: String,
) -> Result<(), String> {
    eprintln!("[copy_formula_to_clipboard] LaTeX: {}", latex);
    eprintln!("[copy_formula_to_clipboard] MathML length: {}", mathml.len());
    clipboard::copy_formula(&latex, &omml, &mathml).map_err(|e| {
        eprintln!("[copy_formula_to_clipboard] FAILED: {}", e);
        e.to_string()
    })
}

#[tauri::command]
async fn copy_latex_to_clipboard(latex: String) -> Result<(), String> {
    clipboard::copy_latex(&latex).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_history(record: HistoryRecord) -> Result<i64, String> {
    history::save(&record).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_history(query: String) -> Result<Vec<HistoryRecord>, String> {
    history::search(&query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_favorite(id: i64) -> Result<(), String> {
    history::toggle_favorite(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_tex(ids: Vec<i64>, options: TexExportOptions) -> Result<Vec<u8>, String> {
    let records = history::get_by_ids(&ids).map_err(|e| e.to_string())?;
    export::export_tex(&records, &options).map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_docx(ids: Vec<i64>) -> Result<Vec<u8>, String> {
    let records = history::get_by_ids(&ids).map_err(|e| e.to_string())?;
    export::export_docx(&records).map_err(|e| e.to_string())
}

// ============================================================
// Tauri App Builder
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            capture_screenshot,
            capture_screen_region,
            cancel_capture,
            recognize_formula,
            convert_to_omml,
            convert_to_mathml,
            copy_formula_to_clipboard,
            copy_latex_to_clipboard,
            save_history,
            search_history,
            toggle_favorite,
            export_tex,
            export_docx,
        ])
        .setup(|app| {
            // Initialize the SQLite database for history records.
            // The database file is stored in the app's data directory
            // (e.g. %APPDATA%/com.formulasnap.app/ on Windows).
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");

            // Ensure the app data directory exists
            std::fs::create_dir_all(&app_data_dir)
                .expect("failed to create app data directory");

            let db_path = app_data_dir.join("history.db");
            let db_path_str = db_path
                .to_str()
                .expect("app data directory path is not valid UTF-8");

            history::init_db(db_path_str)
                .expect("failed to initialize history database");

            // Note: OCR engine initialization is deferred to the first
            // recognize_formula call because the model file may not be
            // present during development/testing. In production, the model
            // path should be resolved relative to the app's resource directory.

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
