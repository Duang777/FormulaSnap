// useImageInput - Custom hook for image input via paste (Ctrl+V) and drag-and-drop
// Validates: Requirements 2.1, 2.2, 2.3

import { useEffect, useCallback, DragEvent } from "react";
import { useFormulaStore } from "../store/formulaStore";

/** Valid image MIME types accepted by FormulaSnap */
export const VALID_IMAGE_TYPES = [
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/bmp",
  "image/webp",
] as const;

/** Error message for invalid image input */
export const INVALID_IMAGE_ERROR =
  "无效的图片格式。请粘贴或拖拽 PNG、JPEG、GIF、BMP 或 WebP 格式的图片。";

/**
 * Check if a MIME type is a valid image type.
 */
export function isValidImageType(mimeType: string): boolean {
  return VALID_IMAGE_TYPES.includes(mimeType as (typeof VALID_IMAGE_TYPES)[number]);
}

/**
 * Read a File/Blob as a number[] (Array.from(Uint8Array)) for passing to recognizeFormula.
 * Uses FileReader for broader environment compatibility (including jsdom).
 */
export function readFileAsNumberArray(file: Blob): Promise<number[]> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (reader.result instanceof ArrayBuffer) {
        resolve(Array.from(new Uint8Array(reader.result)));
      } else {
        reject(new Error("Failed to read file as ArrayBuffer"));
      }
    };
    reader.onerror = () => {
      reject(new Error("Failed to read file"));
    };
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Handle paste event: extract image from clipboard data.
 * Returns the image Blob if found, or null.
 */
export function extractImageFromClipboard(
  clipboardData: DataTransfer | null
): Blob | null {
  if (!clipboardData) return null;

  const items = clipboardData.items;
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.kind === "file" && isValidImageType(item.type)) {
      return item.getAsFile();
    }
  }
  return null;
}

/**
 * Check if clipboard data contains any non-image file items.
 * Used to determine if we should show an error (user tried to paste something non-image).
 */
export function hasNonImageFileItems(
  clipboardData: DataTransfer | null
): boolean {
  if (!clipboardData) return false;

  const items = clipboardData.items;
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.kind === "file" && !isValidImageType(item.type)) {
      return true;
    }
  }
  return false;
}

/**
 * Extract valid image files from a drop event's DataTransfer.
 * Returns the first valid image File, or null.
 */
export function extractImageFromDrop(
  dataTransfer: DataTransfer | null
): File | null {
  if (!dataTransfer) return null;

  const files = dataTransfer.files;
  for (let i = 0; i < files.length; i++) {
    const file = files[i];
    if (isValidImageType(file.type)) {
      return file;
    }
  }
  return null;
}

/**
 * Check if drop data contains any non-image files.
 */
export function hasNonImageDropFiles(
  dataTransfer: DataTransfer | null
): boolean {
  if (!dataTransfer) return false;

  const files = dataTransfer.files;
  for (let i = 0; i < files.length; i++) {
    if (!isValidImageType(files[i].type)) {
      return true;
    }
  }
  return false;
}

/**
 * Custom hook that provides image input via paste and drag-and-drop.
 *
 * - Listens for `paste` events on the document (Ctrl+V with image data)
 * - Returns drag event handlers (onDragOver, onDrop) to attach to a drop zone
 *
 * Validates: Requirements 2.1, 2.2, 2.3
 */
export function useImageInput() {
  const recognizeFormula = useFormulaStore((s) => s.recognizeFormula);
  const setError = useFormulaStore((s) => s.setError);
  const setScreenshotData = useFormulaStore((s) => s.setScreenshotData);

  /**
   * Process a valid image blob: read it and send to OCR.
   */
  const processImage = useCallback(
    async (blob: Blob) => {
      try {
        const imageData = await readFileAsNumberArray(blob);
        const uint8 = new Uint8Array(imageData);
        setScreenshotData(uint8);
        await recognizeFormula(imageData);
      } catch (err) {
        // recognizeFormula already sets error in the store on failure,
        // but if readFileAsNumberArray fails, we handle it here
        if (err instanceof Error && !useFormulaStore.getState().error) {
          setError(err.message);
        }
      }
    },
    [recognizeFormula, setError, setScreenshotData]
  );

  /**
   * Handle paste events on the document.
   * Req 2.1: Ctrl+V with image → OCR
   * Req 2.3: Non-image paste → error
   */
  const handlePaste = useCallback(
    async (event: ClipboardEvent) => {
      const clipboardData = event.clipboardData;

      // Try to extract a valid image
      const imageBlob = extractImageFromClipboard(clipboardData);
      if (imageBlob) {
        event.preventDefault();
        await processImage(imageBlob);
        return;
      }

      // If there are non-image file items, show error (Req 2.3)
      if (hasNonImageFileItems(clipboardData)) {
        event.preventDefault();
        setError(INVALID_IMAGE_ERROR);
        return;
      }

      // If no file items at all (e.g., text paste), let it pass through
      // so other handlers (like text input fields) can handle it
    },
    [processImage, setError]
  );

  // Register paste listener on document
  useEffect(() => {
    document.addEventListener("paste", handlePaste);
    return () => {
      document.removeEventListener("paste", handlePaste);
    };
  }, [handlePaste]);

  /**
   * Drag over handler - must prevent default to allow drop.
   */
  const onDragOver = useCallback((event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  /**
   * Drop handler - extract image from dropped files.
   * Req 2.2: Drop image file → OCR
   * Req 2.3: Non-image drop → error
   */
  const onDrop = useCallback(
    async (event: DragEvent) => {
      event.preventDefault();
      event.stopPropagation();

      const dataTransfer = event.dataTransfer;

      // Try to extract a valid image
      const imageFile = extractImageFromDrop(dataTransfer);
      if (imageFile) {
        await processImage(imageFile);
        return;
      }

      // If files were dropped but none are valid images, show error (Req 2.3)
      if (
        dataTransfer &&
        dataTransfer.files.length > 0 &&
        hasNonImageDropFiles(dataTransfer)
      ) {
        setError(INVALID_IMAGE_ERROR);
        return;
      }

      // If files were dropped but empty (shouldn't happen), or no files at all
      if (dataTransfer && dataTransfer.files.length > 0) {
        setError(INVALID_IMAGE_ERROR);
      }
    },
    [processImage, setError]
  );

  return {
    onDragOver,
    onDrop,
  };
}
