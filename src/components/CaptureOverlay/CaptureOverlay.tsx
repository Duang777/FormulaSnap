// CaptureOverlay - 截图覆盖层组件
// 全屏透明覆盖层，十字光标框选
// Requirements: 1.1 (capture mode with crosshair cursor), 1.3 (overlay on top of all windows)

import { useCallback, useEffect, useRef, useState } from "react";
import type { CaptureOverlayProps } from "../../types";

export type { CaptureOverlayProps };

/** Selection rectangle state during drag */
interface SelectionRect {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
}

/**
 * Compute the normalized rectangle (positive width/height, top-left origin)
 * from a selection that may have been dragged in any direction.
 */
export function normalizeRect(sel: SelectionRect): {
  x: number;
  y: number;
  width: number;
  height: number;
} {
  const x = Math.min(sel.startX, sel.currentX);
  const y = Math.min(sel.startY, sel.currentY);
  const width = Math.abs(sel.currentX - sel.startX);
  const height = Math.abs(sel.currentY - sel.startY);
  return { x, y, width, height };
}

/**
 * CaptureOverlay renders a fullscreen transparent overlay that allows the user
 * to select a screen region by clicking and dragging. The overlay uses a crosshair
 * cursor and draws a selection rectangle with a semi-transparent border.
 *
 * - When the user completes a selection (mouseup with non-zero area), onCapture is called.
 * - When the user presses Escape, onCancel is called.
 * - When isActive is false, the component renders nothing.
 */
export function CaptureOverlay({ isActive, onCapture, onCancel }: CaptureOverlayProps) {
  const [selection, setSelection] = useState<SelectionRect | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const overlayRef = useRef<HTMLDivElement>(null);

  // Reset state when overlay becomes inactive
  useEffect(() => {
    if (!isActive) {
      setSelection(null);
      setIsDragging(false);
    }
  }, [isActive]);

  // Handle Escape key to cancel capture
  useEffect(() => {
    if (!isActive) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setSelection(null);
        setIsDragging(false);
        onCancel();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isActive, onCancel]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only respond to left mouse button
      if (e.button !== 0) return;
      e.preventDefault();

      const rect = overlayRef.current?.getBoundingClientRect();
      const x = rect ? e.clientX - rect.left : e.clientX;
      const y = rect ? e.clientY - rect.top : e.clientY;

      setSelection({
        startX: x,
        startY: y,
        currentX: x,
        currentY: y,
      });
      setIsDragging(true);
    },
    []
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isDragging || !selection) return;
      e.preventDefault();

      const rect = overlayRef.current?.getBoundingClientRect();
      const x = rect ? e.clientX - rect.left : e.clientX;
      const y = rect ? e.clientY - rect.top : e.clientY;

      setSelection((prev) =>
        prev ? { ...prev, currentX: x, currentY: y } : null
      );
    },
    [isDragging, selection]
  );

  const handleMouseUp = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isDragging || !selection) return;
      e.preventDefault();

      const rect = overlayRef.current?.getBoundingClientRect();
      const x = rect ? e.clientX - rect.left : e.clientX;
      const y = rect ? e.clientY - rect.top : e.clientY;

      const finalSelection: SelectionRect = {
        ...selection,
        currentX: x,
        currentY: y,
      };

      const normalized = normalizeRect(finalSelection);

      setIsDragging(false);
      setSelection(null);

      // Only trigger capture if the selection has a meaningful area
      // (minimum 2x2 pixels to avoid accidental clicks)
      if (normalized.width >= 2 && normalized.height >= 2) {
        // Convert region to Uint8Array encoding the region coordinates
        // The region data is encoded as 4 x Int32 values (x, y, width, height)
        const regionData = new Uint8Array(16);
        const view = new DataView(regionData.buffer);
        view.setInt32(0, Math.round(normalized.x), true);
        view.setInt32(4, Math.round(normalized.y), true);
        view.setInt32(8, Math.round(normalized.width), true);
        view.setInt32(12, Math.round(normalized.height), true);
        onCapture(regionData);
      }
    },
    [isDragging, selection, onCapture]
  );

  // Don't render anything when not active
  if (!isActive) {
    return null;
  }

  // Compute the selection rectangle style for rendering
  const selectionStyle = selection
    ? (() => {
        const norm = normalizeRect(selection);
        return {
          left: norm.x,
          top: norm.y,
          width: norm.width,
          height: norm.height,
        };
      })()
    : null;

  return (
    <div
      ref={overlayRef}
      data-testid="capture-overlay"
      className="fixed inset-0 z-[9999] cursor-crosshair select-none"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.3)" }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      role="dialog"
      aria-label="Screen capture overlay"
    >
      {/* Selection rectangle */}
      {selectionStyle && selectionStyle.width > 0 && selectionStyle.height > 0 && (
        <div
          data-testid="capture-selection"
          className="absolute border-2 border-blue-400 pointer-events-none"
          style={{
            left: selectionStyle.left,
            top: selectionStyle.top,
            width: selectionStyle.width,
            height: selectionStyle.height,
            backgroundColor: "rgba(59, 130, 246, 0.1)",
          }}
        />
      )}

      {/* Instruction hint */}
      {!isDragging && (
        <div
          data-testid="capture-hint"
          className="absolute top-4 left-1/2 -translate-x-1/2 px-4 py-2 rounded-lg text-white text-sm pointer-events-none"
          style={{ backgroundColor: "rgba(0, 0, 0, 0.7)" }}
        >
          拖拽选择截图区域，按 Esc 取消
        </div>
      )}
    </div>
  );
}

export default CaptureOverlay;
