import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { FormulaEditor } from "./FormulaEditor";
import { renderHook, act } from "@testing-library/react";
import { useUndoRedo } from "./useUndoRedo";

// ============================================================
// useUndoRedo hook tests
// ============================================================

describe("useUndoRedo", () => {
  it("undo restores previous state", () => {
    const onChange = vi.fn();
    let currentValue = "initial";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    // Push a new state (simulating user typing "hello")
    act(() => {
      result.current.pushState("hello");
    });
    currentValue = "hello";
    rerender();

    // Undo should restore "initial"
    act(() => {
      result.current.undo();
    });

    expect(onChange).toHaveBeenCalledWith("initial");
  });

  it("redo restores undone state", () => {
    const onChange = vi.fn();
    let currentValue = "initial";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    // Push "hello"
    act(() => {
      result.current.pushState("hello");
    });
    currentValue = "hello";
    rerender();

    // Undo back to "initial"
    act(() => {
      result.current.undo();
    });
    currentValue = "initial";
    rerender();

    // Redo should restore "hello"
    act(() => {
      result.current.redo();
    });

    expect(onChange).toHaveBeenLastCalledWith("hello");
  });

  it("undo does nothing when history is empty", () => {
    const onChange = vi.fn();

    const { result } = renderHook(() => useUndoRedo("initial", onChange));

    act(() => {
      result.current.undo();
    });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("redo does nothing when future is empty", () => {
    const onChange = vi.fn();

    const { result } = renderHook(() => useUndoRedo("initial", onChange));

    act(() => {
      result.current.redo();
    });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("new input clears redo stack", () => {
    const onChange = vi.fn();
    let currentValue = "a";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    // Push "b"
    act(() => {
      result.current.pushState("b");
    });
    currentValue = "b";
    rerender();

    // Undo to "a"
    act(() => {
      result.current.undo();
    });
    currentValue = "a";
    rerender();

    // Push "c" (should clear redo stack)
    act(() => {
      result.current.pushState("c");
    });
    currentValue = "c";
    rerender();

    // Redo should do nothing since future was cleared
    onChange.mockClear();
    act(() => {
      result.current.redo();
    });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("supports multiple undo/redo steps", () => {
    const onChange = vi.fn();
    let currentValue = "a";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    // Push b, c, d
    act(() => result.current.pushState("b"));
    currentValue = "b";
    rerender();

    act(() => result.current.pushState("c"));
    currentValue = "c";
    rerender();

    act(() => result.current.pushState("d"));
    currentValue = "d";
    rerender();

    // Undo 3 times: d -> c -> b -> a
    act(() => result.current.undo());
    expect(onChange).toHaveBeenLastCalledWith("c");
    currentValue = "c";
    rerender();

    act(() => result.current.undo());
    expect(onChange).toHaveBeenLastCalledWith("b");
    currentValue = "b";
    rerender();

    act(() => result.current.undo());
    expect(onChange).toHaveBeenLastCalledWith("a");
    currentValue = "a";
    rerender();

    // Redo 2 times: a -> b -> c
    act(() => result.current.redo());
    expect(onChange).toHaveBeenLastCalledWith("b");
    currentValue = "b";
    rerender();

    act(() => result.current.redo());
    expect(onChange).toHaveBeenLastCalledWith("c");
  });

  it("canUndo returns correct state", () => {
    const onChange = vi.fn();
    let currentValue = "a";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    expect(result.current.canUndo()).toBe(false);

    act(() => result.current.pushState("b"));
    currentValue = "b";
    rerender();

    expect(result.current.canUndo()).toBe(true);
  });

  it("canRedo returns correct state", () => {
    const onChange = vi.fn();
    let currentValue = "a";

    const { result, rerender } = renderHook(() =>
      useUndoRedo(currentValue, onChange)
    );

    expect(result.current.canRedo()).toBe(false);

    act(() => result.current.pushState("b"));
    currentValue = "b";
    rerender();

    act(() => result.current.undo());
    currentValue = "a";
    rerender();

    expect(result.current.canRedo()).toBe(true);
  });
});

// ============================================================
// FormulaEditor component tests
// ============================================================

describe("FormulaEditor", () => {
  it("renders textarea with provided latex value", () => {
    render(
      <FormulaEditor
        latex="x^2"
        onChange={vi.fn()}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    const textarea = screen.getByRole("textbox", { name: /latex editor/i });
    expect(textarea).toBeInTheDocument();
    expect(textarea).toHaveValue("x^2");
  });

  it("calls onChange when text is modified", () => {
    const onChange = vi.fn();
    render(
      <FormulaEditor
        latex=""
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    const textarea = screen.getByRole("textbox", { name: /latex editor/i });
    fireEvent.change(textarea, { target: { value: "y^3" } });

    expect(onChange).toHaveBeenCalledWith("y^3");
  });

  it("renders wrap mode selector with correct value", () => {
    render(
      <FormulaEditor
        latex=""
        onChange={vi.fn()}
        wrapMode="display"
        onWrapModeChange={vi.fn()}
      />
    );

    const select = screen.getByRole("combobox", { name: /wrap mode/i });
    expect(select).toHaveValue("display");
  });

  it("calls onWrapModeChange when wrap mode is changed", () => {
    const onWrapModeChange = vi.fn();
    render(
      <FormulaEditor
        latex=""
        onChange={vi.fn()}
        wrapMode="inline"
        onWrapModeChange={onWrapModeChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /wrap mode/i });
    fireEvent.change(select, { target: { value: "display" } });

    expect(onWrapModeChange).toHaveBeenCalledWith("display");
  });

  it("handles Ctrl+Z for undo", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <FormulaEditor
        latex=""
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    const textarea = screen.getByRole("textbox", { name: /latex editor/i });

    // Type something to create history
    fireEvent.change(textarea, { target: { value: "hello" } });

    // Re-render with new value
    rerender(
      <FormulaEditor
        latex="hello"
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    // Press Ctrl+Z
    fireEvent.keyDown(textarea, { key: "z", ctrlKey: true });

    // Should have called onChange with the previous value
    expect(onChange).toHaveBeenLastCalledWith("");
  });

  it("handles Ctrl+Y for redo", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <FormulaEditor
        latex=""
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    const textarea = screen.getByRole("textbox", { name: /latex editor/i });

    // Type something
    fireEvent.change(textarea, { target: { value: "hello" } });
    rerender(
      <FormulaEditor
        latex="hello"
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    // Undo
    fireEvent.keyDown(textarea, { key: "z", ctrlKey: true });
    rerender(
      <FormulaEditor
        latex=""
        onChange={onChange}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    // Redo with Ctrl+Y
    fireEvent.keyDown(textarea, { key: "y", ctrlKey: true });

    expect(onChange).toHaveBeenLastCalledWith("hello");
  });

  it("has spellCheck disabled on textarea", () => {
    render(
      <FormulaEditor
        latex=""
        onChange={vi.fn()}
        wrapMode="inline"
        onWrapModeChange={vi.fn()}
      />
    );

    const textarea = screen.getByRole("textbox", { name: /latex editor/i });
    expect(textarea).toHaveAttribute("spellcheck", "false");
  });
});
