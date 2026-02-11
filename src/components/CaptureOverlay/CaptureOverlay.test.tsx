import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CaptureOverlay, normalizeRect } from "./CaptureOverlay";

// ============================================================
// normalizeRect unit tests (pure function)
// ============================================================

describe("normalizeRect", () => {
  it("returns correct rect when dragged top-left to bottom-right", () => {
    const result = normalizeRect({
      startX: 10,
      startY: 20,
      currentX: 110,
      currentY: 120,
    });
    expect(result).toEqual({ x: 10, y: 20, width: 100, height: 100 });
  });

  it("returns correct rect when dragged bottom-right to top-left", () => {
    const result = normalizeRect({
      startX: 110,
      startY: 120,
      currentX: 10,
      currentY: 20,
    });
    expect(result).toEqual({ x: 10, y: 20, width: 100, height: 100 });
  });

  it("returns correct rect when dragged top-right to bottom-left", () => {
    const result = normalizeRect({
      startX: 200,
      startY: 50,
      currentX: 100,
      currentY: 150,
    });
    expect(result).toEqual({ x: 100, y: 50, width: 100, height: 100 });
  });

  it("returns zero-size rect when start equals current", () => {
    const result = normalizeRect({
      startX: 50,
      startY: 50,
      currentX: 50,
      currentY: 50,
    });
    expect(result).toEqual({ x: 50, y: 50, width: 0, height: 0 });
  });
});

// ============================================================
// CaptureOverlay component tests
// ============================================================

describe("CaptureOverlay", () => {
  it("renders nothing when isActive is false", () => {
    const { container } = render(
      <CaptureOverlay isActive={false} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders overlay when isActive is true", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");
    expect(overlay).toBeInTheDocument();
  });

  it("has crosshair cursor class", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");
    expect(overlay.className).toContain("cursor-crosshair");
  });

  it("has fixed positioning and full-screen coverage", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");
    expect(overlay.className).toContain("fixed");
    expect(overlay.className).toContain("inset-0");
  });

  it("has high z-index for overlay on top of all windows", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");
    expect(overlay.className).toContain("z-[9999]");
  });

  it("shows instruction hint when not dragging", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const hint = screen.getByTestId("capture-hint");
    expect(hint).toBeInTheDocument();
    expect(hint.textContent).toContain("Esc");
  });

  it("calls onCancel when Escape key is pressed", () => {
    const onCancel = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={onCancel} />
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("does not call onCancel on Escape when inactive", () => {
    const onCancel = vi.fn();
    render(
      <CaptureOverlay isActive={false} onCapture={vi.fn()} onCancel={onCancel} />
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("shows selection rectangle during drag", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Start drag
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 0 });

    // Move mouse to create selection
    fireEvent.mouseMove(overlay, { clientX: 200, clientY: 200 });

    // Selection rectangle should be visible
    const selection = screen.getByTestId("capture-selection");
    expect(selection).toBeInTheDocument();
  });

  it("calls onCapture with region data on mouseup after valid selection", () => {
    const onCapture = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={onCapture} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Simulate drag from (100, 100) to (200, 200)
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(overlay, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(overlay, { clientX: 200, clientY: 200 });

    expect(onCapture).toHaveBeenCalledTimes(1);

    // Verify the captured data is a Uint8Array with 16 bytes (4 x Int32)
    const capturedData = onCapture.mock.calls[0][0];
    expect(capturedData).toBeInstanceOf(Uint8Array);
    expect(capturedData.length).toBe(16);

    // Decode the region coordinates
    const view = new DataView(capturedData.buffer);
    const x = view.getInt32(0, true);
    const y = view.getInt32(4, true);
    const width = view.getInt32(8, true);
    const height = view.getInt32(12, true);

    expect(x).toBe(100);
    expect(y).toBe(100);
    expect(width).toBe(100);
    expect(height).toBe(100);
  });

  it("does not call onCapture for tiny selections (accidental clicks)", () => {
    const onCapture = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={onCapture} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Simulate a click (no drag, same position)
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseUp(overlay, { clientX: 100, clientY: 100 });

    expect(onCapture).not.toHaveBeenCalled();
  });

  it("does not call onCapture for selection smaller than 2x2", () => {
    const onCapture = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={onCapture} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Simulate a very small drag (1px)
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseUp(overlay, { clientX: 101, clientY: 101 });

    expect(onCapture).not.toHaveBeenCalled();
  });

  it("ignores right mouse button clicks", () => {
    const onCapture = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={onCapture} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Right click should not start selection
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 2 });
    fireEvent.mouseUp(overlay, { clientX: 200, clientY: 200 });

    expect(onCapture).not.toHaveBeenCalled();
  });

  it("handles reverse drag direction (bottom-right to top-left)", () => {
    const onCapture = vi.fn();
    render(
      <CaptureOverlay isActive={true} onCapture={onCapture} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Drag from (300, 300) to (100, 100) - reverse direction
    fireEvent.mouseDown(overlay, { clientX: 300, clientY: 300, button: 0 });
    fireEvent.mouseMove(overlay, { clientX: 100, clientY: 100 });
    fireEvent.mouseUp(overlay, { clientX: 100, clientY: 100 });

    expect(onCapture).toHaveBeenCalledTimes(1);

    const capturedData = onCapture.mock.calls[0][0];
    const view = new DataView(capturedData.buffer);
    const x = view.getInt32(0, true);
    const y = view.getInt32(4, true);
    const width = view.getInt32(8, true);
    const height = view.getInt32(12, true);

    // Should normalize to top-left origin
    expect(x).toBe(100);
    expect(y).toBe(100);
    expect(width).toBe(200);
    expect(height).toBe(200);
  });

  it("clears selection after capture completes", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByTestId("capture-overlay");

    // Complete a selection
    fireEvent.mouseDown(overlay, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(overlay, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(overlay, { clientX: 200, clientY: 200 });

    // Selection rectangle should be gone
    expect(screen.queryByTestId("capture-selection")).not.toBeInTheDocument();
  });

  it("has accessible role and label", () => {
    render(
      <CaptureOverlay isActive={true} onCapture={vi.fn()} onCancel={vi.fn()} />
    );
    const overlay = screen.getByRole("dialog");
    expect(overlay).toHaveAttribute("aria-label", "Screen capture overlay");
  });
});
