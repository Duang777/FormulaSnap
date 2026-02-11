import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ActionBar } from "./ActionBar";

describe("ActionBar", () => {
  const defaultProps = {
    onCopyToWord: vi.fn(),
    onCopyLatex: vi.fn(),
    onRetry: vi.fn(),
    onSave: vi.fn(),
    isConverting: false,
  };

  it("renders all four action buttons", () => {
    render(<ActionBar {...defaultProps} />);

    expect(screen.getByRole("button", { name: "复制到 Word" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "复制 LaTeX" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeInTheDocument();
  });

  it("calls onCopyToWord when '复制到 Word' button is clicked", () => {
    const onCopyToWord = vi.fn();
    render(<ActionBar {...defaultProps} onCopyToWord={onCopyToWord} />);

    fireEvent.click(screen.getByRole("button", { name: "复制到 Word" }));
    expect(onCopyToWord).toHaveBeenCalledTimes(1);
  });

  it("calls onCopyLatex when '复制 LaTeX' button is clicked", () => {
    const onCopyLatex = vi.fn();
    render(<ActionBar {...defaultProps} onCopyLatex={onCopyLatex} />);

    fireEvent.click(screen.getByRole("button", { name: "复制 LaTeX" }));
    expect(onCopyLatex).toHaveBeenCalledTimes(1);
  });

  it("calls onRetry when '重试' button is clicked", () => {
    const onRetry = vi.fn();
    render(<ActionBar {...defaultProps} onRetry={onRetry} />);

    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("calls onSave when '保存' button is clicked", () => {
    const onSave = vi.fn();
    render(<ActionBar {...defaultProps} onSave={onSave} />);

    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("disables '复制到 Word' button when isConverting is true", () => {
    render(<ActionBar {...defaultProps} isConverting={true} />);

    const copyToWordButton = screen.getByRole("button", { name: "复制到 Word" });
    expect(copyToWordButton).toBeDisabled();
  });

  it("shows loading spinner when isConverting is true", () => {
    render(<ActionBar {...defaultProps} isConverting={true} />);

    expect(screen.getByRole("status", { name: "转换中" })).toBeInTheDocument();
    expect(screen.getByText("转换中...")).toBeInTheDocument();
  });

  it("does not call onCopyToWord when button is disabled (isConverting)", () => {
    const onCopyToWord = vi.fn();
    render(
      <ActionBar {...defaultProps} onCopyToWord={onCopyToWord} isConverting={true} />
    );

    const button = screen.getByRole("button", { name: "复制到 Word" });
    fireEvent.click(button);
    expect(onCopyToWord).not.toHaveBeenCalled();
  });

  it("other buttons remain enabled when isConverting is true", () => {
    render(<ActionBar {...defaultProps} isConverting={true} />);

    expect(screen.getByRole("button", { name: "复制 LaTeX" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "重试" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "保存" })).not.toBeDisabled();
  });

  it("shows '复制到 Word' text when not converting", () => {
    render(<ActionBar {...defaultProps} isConverting={false} />);

    expect(screen.getByText("复制到 Word")).toBeInTheDocument();
    expect(screen.queryByText("转换中...")).not.toBeInTheDocument();
  });
});
