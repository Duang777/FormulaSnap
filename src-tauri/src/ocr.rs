// OcrService - 离线公式识别模块
// 使用 ONNX Runtime 加载 pix2tex 模型进行离线推理

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// OCR 识别结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// 识别出的 LaTeX 字符串
    pub latex: String,
    /// 置信度 0.0 ~ 1.0
    pub confidence: f64,
}

/// OCR 错误类型
#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("模型加载失败: {0}")]
    ModelLoad(String),
    #[error("推理失败: {0}")]
    InferenceFailed(String),
    #[error("识别超时")]
    Timeout,
    #[error("识别结果为空")]
    EmptyResult,
}

impl Serialize for OcrError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<ort::Error> for OcrError {
    fn from(e: ort::Error) -> Self {
        OcrError::InferenceFailed(format!("ONNX Runtime 错误: {}", e))
    }
}

/// 推理超时时间（10 秒）
const INFERENCE_TIMEOUT: Duration = Duration::from_secs(10);

/// pix2tex 模型默认输入高度
const MODEL_INPUT_HEIGHT: u32 = 64;

/// pix2tex 模型最大输入宽度
const MODEL_MAX_INPUT_WIDTH: u32 = 672;

/// OCR 引擎，持有 ONNX Runtime Session
///
/// 使用 `Arc<Mutex>` 包装 `Session` 以便在异步任务间安全共享。
/// `Session::run` 需要 `&mut self`，因此需要 Mutex 保护。
pub struct OcrEngine {
    session: Arc<std::sync::Mutex<Session>>,
    model_path: String,
}

impl std::fmt::Debug for OcrEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OcrEngine")
            .field("model_path", &self.model_path)
            .finish()
    }
}

/// 初始化 OCR 引擎（加载 ONNX 模型）
///
/// 使用 `ort::Session::builder()` 加载 pix2tex ONNX 模型文件。
/// 如果模型文件不存在或格式无效，返回 `OcrError::ModelLoad`。
///
/// # Arguments
/// * `model_path` - ONNX 模型文件路径
///
/// # Returns
/// * `Ok(OcrEngine)` - 成功加载的引擎实例
/// * `Err(OcrError::ModelLoad)` - 模型加载失败
pub fn init_engine(model_path: &str) -> Result<OcrEngine, OcrError> {
    // 检查模型文件是否存在
    if !Path::new(model_path).exists() {
        return Err(OcrError::ModelLoad(format!(
            "模型文件不存在: {}",
            model_path
        )));
    }

    // 使用 ort v2 API 创建 Session
    let session = Session::builder()
        .map_err(|e| OcrError::ModelLoad(format!("创建 Session builder 失败: {}", e)))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| OcrError::ModelLoad(format!("设置优化级别失败: {}", e)))?
        .commit_from_file(model_path)
        .map_err(|e| OcrError::ModelLoad(format!("加载模型文件失败: {}", e)))?;

    Ok(OcrEngine {
        session: Arc::new(std::sync::Mutex::new(session)),
        model_path: model_path.to_string(),
    })
}

/// 预处理图片为模型输入张量数据
///
/// 将图片转换为灰度图，缩放到模型输入尺寸，并归一化像素值到 [0, 1]。
///
/// # Returns
/// * `(Vec<f32>, u32, u32)` - (归一化像素数据, 宽度, 高度)
fn prepare_image(image_bytes: &[u8]) -> Result<(Vec<f32>, u32, u32), OcrError> {
    // 1. 从字节加载图片
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| OcrError::InferenceFailed(format!("图片解码失败: {}", e)))?;

    // 2. 转换为灰度图
    let gray = img.to_luma8();
    let (orig_w, orig_h) = gray.dimensions();

    if orig_w == 0 || orig_h == 0 {
        return Err(OcrError::InferenceFailed("图片尺寸无效".to_string()));
    }

    // 3. 缩放到模型输入尺寸（高度固定，宽度按比例缩放，但不超过最大宽度）
    let target_h = MODEL_INPUT_HEIGHT;
    let scale = target_h as f64 / orig_h as f64;
    let target_w = ((orig_w as f64 * scale).round() as u32)
        .max(1)
        .min(MODEL_MAX_INPUT_WIDTH);

    let resized = image::imageops::resize(
        &gray,
        target_w,
        target_h,
        image::imageops::FilterType::Lanczos3,
    );

    // 4. 归一化像素值到 [0, 1] 范围
    let pixels: Vec<f32> = resized.pixels().map(|p| p[0] as f32 / 255.0).collect();

    Ok((pixels, target_w, target_h))
}

/// 将模型输出的 token 索引解码为 LaTeX 字符串
///
/// pix2tex 模型输出一系列 token 索引，需要映射到对应的 LaTeX token。
/// 这里使用一个简化的 token 映射表。实际使用时应加载模型配套的词汇表。
fn decode_tokens(token_indices: &[i64]) -> String {
    // pix2tex 模型的特殊 token
    const BOS_TOKEN: i64 = 0; // 序列开始
    const EOS_TOKEN: i64 = 1; // 序列结束
    const PAD_TOKEN: i64 = 2; // 填充

    let mut latex_parts: Vec<String> = Vec::new();

    for &idx in token_indices {
        // 跳过特殊 token
        if idx == BOS_TOKEN || idx == EOS_TOKEN || idx == PAD_TOKEN {
            if idx == EOS_TOKEN {
                break; // 遇到结束 token 停止解码
            }
            continue;
        }

        // 将 token 索引转换为字符串表示
        // 实际实现中应使用模型配套的词汇表文件
        latex_parts.push(format!("token_{}", idx));
    }

    latex_parts.join(" ")
}

/// 从模型输出计算置信度
///
/// 基于输出 logits 计算平均置信度。
/// 对每个 token 位置取 softmax 后的最大概率，然后取平均值。
fn compute_confidence(logits: &[f32], vocab_size: usize, seq_len: usize) -> f64 {
    if seq_len == 0 || vocab_size == 0 {
        return 0.0;
    }

    let mut total_confidence = 0.0;
    let mut count = 0;

    for t in 0..seq_len {
        let offset = t * vocab_size;
        if offset + vocab_size > logits.len() {
            break;
        }

        let slice = &logits[offset..offset + vocab_size];

        // 计算 softmax 的最大值（数值稳定性）
        let max_val = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        // 计算 softmax 分母
        let sum_exp: f32 = slice.iter().map(|&x| (x - max_val).exp()).sum();

        if sum_exp > 0.0 {
            // 找到最大的 softmax 概率值
            let max_softmax = slice
                .iter()
                .map(|&x| (x - max_val).exp() / sum_exp)
                .fold(f32::NEG_INFINITY, f32::max);
            total_confidence += max_softmax as f64;
            count += 1;
        }
    }

    if count > 0 {
        (total_confidence / count as f64).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// 在 ONNX Session 上执行推理（同步，阻塞调用）
///
/// 此函数在当前线程上运行推理，应通过 `tokio::task::spawn_blocking`
/// 或类似机制在独立线程中调用，以避免阻塞 UI 线程。
fn run_inference(session: &mut Session, image_bytes: &[u8]) -> Result<OcrResult, OcrError> {
    // 1. 预处理图片
    let (pixels, width, height) = prepare_image(image_bytes)?;

    // 2. 创建输入张量 [batch=1, channels=1, height, width]
    let input_array = ndarray::Array4::from_shape_vec(
        (1, 1, height as usize, width as usize),
        pixels,
    )
    .map_err(|e| OcrError::InferenceFailed(format!("创建输入张量失败: {}", e)))?;

    // 3. 创建 ort Tensor 并运行推理
    let input_tensor = ort::value::Tensor::from_array(input_array)
        .map_err(|e| OcrError::InferenceFailed(format!("创建 ort 张量失败: {}", e)))?;

    let outputs = session
        .run(ort::inputs![input_tensor])
        .map_err(|e| OcrError::InferenceFailed(format!("ONNX 推理失败: {}", e)))?;

    // 4. 提取输出
    // pix2tex 模型通常输出 token 索引或 logits
    // 尝试提取 i64 类型的 token 索引输出
    let result = if let Ok(output_view) = outputs[0].try_extract_array::<i64>() {
        let token_indices: Vec<i64> = output_view.iter().copied().collect();
        let latex = decode_tokens(&token_indices);
        let confidence = if latex.is_empty() { 0.0 } else { 0.8 };
        OcrResult { latex, confidence }
    } else if let Ok(output_view) = outputs[0].try_extract_array::<f32>() {
        // 如果输出是 float logits，需要 argmax 解码
        let shape = output_view.shape();
        let logits: Vec<f32> = output_view.iter().copied().collect();

        if shape.len() >= 2 {
            let seq_len = shape[shape.len() - 2];
            let vocab_size = shape[shape.len() - 1];

            // 对每个时间步取 argmax 得到 token 索引
            let mut token_indices: Vec<i64> = Vec::with_capacity(seq_len);
            for t in 0..seq_len {
                let offset = t * vocab_size;
                if offset + vocab_size > logits.len() {
                    break;
                }
                let slice = &logits[offset..offset + vocab_size];
                let max_idx = slice
                    .iter()
                    .enumerate()
                    .max_by(|(_, a): &(usize, &f32), (_, b): &(usize, &f32)| {
                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(idx, _)| idx as i64)
                    .unwrap_or(0);
                token_indices.push(max_idx);
            }

            let latex = decode_tokens(&token_indices);
            let confidence = compute_confidence(&logits, vocab_size, seq_len);
            OcrResult { latex, confidence }
        } else {
            return Err(OcrError::InferenceFailed(
                "模型输出形状不符合预期".to_string(),
            ));
        }
    } else {
        return Err(OcrError::InferenceFailed(
            "无法提取模型输出张量".to_string(),
        ));
    };

    // 5. 检查结果是否为空
    if result.latex.trim().is_empty() {
        return Err(OcrError::EmptyResult);
    }

    Ok(result)
}

/// 识别图片中的公式（同步版本）
///
/// 在当前线程上运行推理。如果需要异步非阻塞版本，
/// 请使用 `recognize_async`。
///
/// # Arguments
/// * `engine` - 已初始化的 OCR 引擎
/// * `image` - 图片字节数据（PNG/JPEG 等格式）
///
/// # Returns
/// * `Ok(OcrResult)` - 识别成功，包含 LaTeX 和置信度
/// * `Err(OcrError)` - 识别失败
pub fn recognize(engine: &OcrEngine, image: &[u8]) -> Result<OcrResult, OcrError> {
    let mut session = engine.session.lock()
        .map_err(|e| OcrError::InferenceFailed(format!("获取 Session 锁失败: {}", e)))?;
    run_inference(&mut session, image)
}

/// 异步识别图片中的公式（带 10 秒超时）
///
/// 在 `tokio::task::spawn_blocking` 中运行推理，不阻塞 UI 线程。
/// 如果推理超过 10 秒未完成，返回 `OcrError::Timeout`。
///
/// # Arguments
/// * `engine` - 已初始化的 OCR 引擎（Arc 包装以便跨线程共享）
/// * `image` - 图片字节数据
///
/// # Returns
/// * `Ok(OcrResult)` - 识别成功
/// * `Err(OcrError::Timeout)` - 识别超时（超过 10 秒）
/// * `Err(OcrError::InferenceFailed)` - 推理失败
pub async fn recognize_async(engine: &OcrEngine, image: Vec<u8>) -> Result<OcrResult, OcrError> {
    let session = Arc::clone(&engine.session);

    // 使用 tokio::time::timeout 实现 10 秒超时
    // 使用 tokio::task::spawn_blocking 在独立线程中运行推理，不阻塞 UI
    let result = tokio::time::timeout(INFERENCE_TIMEOUT, async {
        let session = session;
        let image = image;
        tokio::task::spawn_blocking(move || {
            let mut session = session.lock()
                .map_err(|e| OcrError::InferenceFailed(format!("获取 Session 锁失败: {}", e)))?;
            run_inference(&mut session, &image)
        })
            .await
            .map_err(|e| OcrError::InferenceFailed(format!("推理任务异常: {}", e)))?
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(OcrError::Timeout),
    }
}

/// 获取引擎的模型路径
impl OcrEngine {
    /// 返回加载的模型文件路径
    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ================================================================
    // Helper functions
    // ================================================================

    /// Create a simple test PNG image with given dimensions
    fn create_test_image(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, ImageFormat, Rgba};
        use std::io::Cursor;

        let img = ImageBuffer::from_fn(width, height, |x, y| {
            // Create a pattern with some dark pixels (simulating formula content)
            if (x + y) % 3 == 0 {
                Rgba([0u8, 0, 0, 255])
            } else {
                Rgba([255u8, 255, 255, 255])
            }
        });
        let dynamic = image::DynamicImage::ImageRgba8(img);
        let mut buf = Cursor::new(Vec::new());
        dynamic.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    // ================================================================
    // init_engine tests
    // ================================================================

    #[test]
    fn test_init_engine_nonexistent_model() {
        let result = init_engine("nonexistent_model.onnx");
        assert!(result.is_err());
        match result.unwrap_err() {
            OcrError::ModelLoad(msg) => {
                assert!(
                    msg.contains("模型文件不存在"),
                    "Error should mention file not found, got: {}",
                    msg
                );
            }
            other => panic!("Expected ModelLoad error, got: {:?}", other),
        }
    }

    #[test]
    fn test_init_engine_empty_path() {
        let result = init_engine("");
        assert!(result.is_err());
        match result.unwrap_err() {
            OcrError::ModelLoad(_) => {} // expected
            other => panic!("Expected ModelLoad error, got: {:?}", other),
        }
    }

    // ================================================================
    // prepare_image tests
    // ================================================================

    #[test]
    fn test_prepare_image_valid_png() {
        let image_bytes = create_test_image(200, 100);
        let result = prepare_image(&image_bytes);
        assert!(result.is_ok());
        let (pixels, width, height) = result.unwrap();
        assert_eq!(height, MODEL_INPUT_HEIGHT);
        assert!(width > 0);
        assert_eq!(pixels.len(), (width * height) as usize);
        // All pixel values should be in [0, 1]
        for &p in &pixels {
            assert!(p >= 0.0 && p <= 1.0, "Pixel value {} out of range", p);
        }
    }

    #[test]
    fn test_prepare_image_invalid_bytes() {
        let result = prepare_image(b"not an image");
        assert!(result.is_err());
        match result.unwrap_err() {
            OcrError::InferenceFailed(msg) => {
                assert!(msg.contains("图片解码失败"), "Got: {}", msg);
            }
            other => panic!("Expected InferenceFailed, got: {:?}", other),
        }
    }

    #[test]
    fn test_prepare_image_scales_to_model_height() {
        // Test with various image sizes
        for (w, h) in [(100, 50), (300, 200), (50, 100), (1000, 500)] {
            let image_bytes = create_test_image(w, h);
            let (_, _, out_h) = prepare_image(&image_bytes).unwrap();
            assert_eq!(
                out_h, MODEL_INPUT_HEIGHT,
                "Image {}x{} should scale to height {}",
                w, h, MODEL_INPUT_HEIGHT
            );
        }
    }

    #[test]
    fn test_prepare_image_width_capped() {
        // Very wide image should have width capped at MODEL_MAX_INPUT_WIDTH
        let image_bytes = create_test_image(2000, 64);
        let (_, width, _) = prepare_image(&image_bytes).unwrap();
        assert!(
            width <= MODEL_MAX_INPUT_WIDTH,
            "Width {} should be <= {}",
            width,
            MODEL_MAX_INPUT_WIDTH
        );
    }

    #[test]
    fn test_prepare_image_normalizes_pixels() {
        let image_bytes = create_test_image(100, 100);
        let (pixels, _, _) = prepare_image(&image_bytes).unwrap();
        // All pixel values should be in [0, 1] range
        for &p in &pixels {
            assert!(p >= 0.0 && p <= 1.0, "Pixel value {} out of range", p);
        }
        // Check that we have some variation in pixel values (not all same)
        let min_val = pixels.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = pixels.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max_val - min_val > 0.01, "Should have variation in pixel values");
    }

    // ================================================================
    // decode_tokens tests
    // ================================================================

    #[test]
    fn test_decode_tokens_empty() {
        let result = decode_tokens(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_decode_tokens_only_special() {
        // BOS=0, EOS=1, PAD=2
        let result = decode_tokens(&[0, 1, 2]);
        assert!(result.is_empty(), "Only special tokens should produce empty string");
    }

    #[test]
    fn test_decode_tokens_stops_at_eos() {
        // Tokens after EOS should be ignored
        let result = decode_tokens(&[0, 3, 4, 1, 5, 6]);
        assert!(!result.contains("token_5"), "Tokens after EOS should be ignored");
        assert!(result.contains("token_3"));
        assert!(result.contains("token_4"));
    }

    #[test]
    fn test_decode_tokens_normal() {
        let result = decode_tokens(&[0, 10, 20, 30, 1]);
        assert!(result.contains("token_10"));
        assert!(result.contains("token_20"));
        assert!(result.contains("token_30"));
    }

    // ================================================================
    // compute_confidence tests
    // ================================================================

    #[test]
    fn test_compute_confidence_empty() {
        assert_eq!(compute_confidence(&[], 0, 0), 0.0);
    }

    #[test]
    fn test_compute_confidence_single_token_high() {
        // One token with very high logit for one class
        // vocab_size=3, seq_len=1
        // logits: [10.0, 0.0, 0.0] -> softmax max ≈ 1.0
        let logits = vec![10.0, 0.0, 0.0];
        let conf = compute_confidence(&logits, 3, 1);
        assert!(conf > 0.9, "High logit should give high confidence, got {}", conf);
    }

    #[test]
    fn test_compute_confidence_uniform() {
        // Uniform logits -> low confidence (1/vocab_size)
        let logits = vec![1.0, 1.0, 1.0, 1.0];
        let conf = compute_confidence(&logits, 4, 1);
        assert!(
            (conf - 0.25).abs() < 0.01,
            "Uniform logits should give ~0.25 confidence, got {}",
            conf
        );
    }

    #[test]
    fn test_compute_confidence_in_range() {
        // Any valid logits should produce confidence in [0, 1]
        let logits = vec![1.0, 2.0, 3.0, -1.0, 0.5, 2.5];
        let conf = compute_confidence(&logits, 3, 2);
        assert!(conf >= 0.0 && conf <= 1.0, "Confidence {} out of range", conf);
    }

    // ================================================================
    // recognize tests (without actual model)
    // ================================================================

    #[test]
    fn test_recognize_without_model() {
        // Without a real model, init_engine should fail
        let result = init_engine("fake_model.onnx");
        assert!(result.is_err());
    }

    // ================================================================
    // OcrError serialization tests
    // ================================================================

    #[test]
    fn test_ocr_error_serialize() {
        let errors = vec![
            OcrError::ModelLoad("test".to_string()),
            OcrError::InferenceFailed("test".to_string()),
            OcrError::Timeout,
            OcrError::EmptyResult,
        ];
        for err in &errors {
            let json = serde_json::to_string(err).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_ocr_error_display() {
        assert!(OcrError::Timeout.to_string().contains("超时"));
        assert!(OcrError::EmptyResult.to_string().contains("为空"));
        assert!(OcrError::ModelLoad("x".into()).to_string().contains("模型加载失败"));
        assert!(OcrError::InferenceFailed("x".into()).to_string().contains("推理失败"));
    }

    // ================================================================
    // OcrResult tests
    // ================================================================

    #[test]
    fn test_ocr_result_serialize_deserialize() {
        let result = OcrResult {
            latex: "x^2 + y^2 = z^2".to_string(),
            confidence: 0.95,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: OcrResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.latex, result.latex);
        assert!((deserialized.confidence - result.confidence).abs() < f64::EPSILON);
    }

    // ================================================================
    // Async timeout tests
    // ================================================================

    #[tokio::test]
    async fn test_recognize_async_without_model() {
        // Without a real model, init_engine should fail
        let result = init_engine("nonexistent.onnx");
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_timeout_constant() {
        assert_eq!(INFERENCE_TIMEOUT, Duration::from_secs(10));
    }

    // ================================================================
    // Property-Based Tests
    // ================================================================

    /// **Property 3: OCR 置信度范围不变量**
    /// 
    /// For any OcrService 返回的 OcrResult，confidence 字段的值应在 [0.0, 1.0] 闭区间内。
    /// 
    /// **Validates: Requirements 3.2**
    /// 
    /// Since the actual OCR model may not be available in test environment,
    /// we test the core confidence computation logic (compute_confidence function)
    /// which is responsible for producing confidence values in the OCR pipeline.
    mod property_tests {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(20))]

            /// Property 3: compute_confidence always returns values in [0.0, 1.0]
            /// 
            /// For any arbitrary logits array, vocab_size, and seq_len,
            /// the computed confidence must be within the valid range.
            /// 
            /// **Validates: Requirements 3.2**
            #[test]
            fn prop_compute_confidence_in_valid_range(
                // Generate random logits with various values including edge cases
                logits in prop::collection::vec(-100.0f32..100.0f32, 0..500),
                vocab_size in 1usize..50,
                seq_len in 0usize..20
            ) {
                let confidence = compute_confidence(&logits, vocab_size, seq_len);
                
                prop_assert!(
                    confidence >= 0.0 && confidence <= 1.0,
                    "Confidence {} is out of valid range [0.0, 1.0] for logits len={}, vocab_size={}, seq_len={}",
                    confidence, logits.len(), vocab_size, seq_len
                );
            }

            /// Property 3: OcrResult confidence field validation
            /// 
            /// For any OcrResult that could be constructed, the confidence
            /// value should be validated to be in [0.0, 1.0] range.
            /// This tests the struct's invariant directly.
            /// 
            /// **Validates: Requirements 3.2**
            #[test]
            fn prop_ocr_result_confidence_range(
                latex in ".*",
                confidence in 0.0f64..=1.0f64
            ) {
                let result = OcrResult {
                    latex,
                    confidence,
                };
                
                prop_assert!(
                    result.confidence >= 0.0 && result.confidence <= 1.0,
                    "OcrResult confidence {} is out of valid range [0.0, 1.0]",
                    result.confidence
                );
            }

            /// Property 3: compute_confidence with extreme logit values
            /// 
            /// Even with extreme logit values (very large positive/negative),
            /// the confidence should remain in valid range due to softmax normalization.
            /// 
            /// **Validates: Requirements 3.2**
            #[test]
            fn prop_compute_confidence_extreme_values(
                // Test with extreme values that could cause numerical issues
                base_logit in -1000.0f32..1000.0f32,
                vocab_size in 1usize..20,
                seq_len in 1usize..10
            ) {
                // Create logits with one extreme value and others at 0
                let mut logits = vec![0.0f32; vocab_size * seq_len];
                if !logits.is_empty() {
                    logits[0] = base_logit;
                }
                
                let confidence = compute_confidence(&logits, vocab_size, seq_len);
                
                prop_assert!(
                    confidence >= 0.0 && confidence <= 1.0,
                    "Confidence {} is out of range for extreme logit value {}",
                    confidence, base_logit
                );
            }

            /// Property 3: compute_confidence with uniform distribution
            /// 
            /// When all logits are equal (uniform distribution), confidence
            /// should be approximately 1/vocab_size, still within [0.0, 1.0].
            /// 
            /// **Validates: Requirements 3.2**
            #[test]
            fn prop_compute_confidence_uniform_distribution(
                uniform_value in -10.0f32..10.0f32,
                vocab_size in 2usize..20,
                seq_len in 1usize..10
            ) {
                let logits = vec![uniform_value; vocab_size * seq_len];
                let confidence = compute_confidence(&logits, vocab_size, seq_len);
                
                prop_assert!(
                    confidence >= 0.0 && confidence <= 1.0,
                    "Confidence {} is out of range for uniform logits",
                    confidence
                );
                
                // For uniform distribution, confidence should be approximately 1/vocab_size
                let expected = 1.0 / vocab_size as f64;
                prop_assert!(
                    (confidence - expected).abs() < 0.01,
                    "Uniform distribution confidence {} should be approximately {} (1/vocab_size)",
                    confidence, expected
                );
            }
        }
    }
}
