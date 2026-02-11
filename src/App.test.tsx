import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import App from "./App";
import { useFormulaStore } from "./store/formulaStore";

// ============================================================
// Mock @tauri-apps/api/core invoke
// ============================================================

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// ============================================================
// Mock KaTeX to avoid rendering issues in jsdom
// ============================================================

vi.mock("katex", () => ({
  default: {
    renderToString: (latex: string) => {
      if (!latex) return "";
      if (latex.includes("\\invalid")) {
        throw new Error("KaTeX parse error");
      }
      return `<span class="katex">${latex}</span>`;
    },
  },
}));

// Reset store state before each test
beforeEach(() => {
  vi.clearAllMocks();
  useFormulaStore.setState({
    currentLatex: "",
    originalLatex: "",
    confidence: 0,
    screenshotData: null,
    wrapMode: "inline",
    isCapturing: false,
    isRecognizing: false,
    isConverting: false,
    error: null,
    historyRecords: [],
    searchQuery: "",
  });
  // Mock search_history to return empty array by default
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "search_history") return Promise.resolve([]);
    return Promise.resolve(null);
  });
});

describe("App Shell", () => {
  it("renders without crashing", () => {
    render(<App />);
    expect(screen.getByTestId("app-shell")).toBeInTheDocument();
  });

  it("renders the header with FormulaSnap title", () => {
    render(<App />);
    expect(screen.getByText("FormulaSnap")).toBeInTheDocument();
  });

  it("renders the capture button", () => {
    render(<App />);
    expect(screen.getByTestId("capture-btn")).toBeInTheDocument();
  });

  it("renders export buttons", () => {
    render(<App />);
    expect(screen.getByTestId("export-tex-btn")).toBeInTheDocument();
    expect(screen.getByTestId("export-docx-btn")).toBeInTheDocument();
  });

  it("renders the sidebar toggle button", () => {
    render(<App />);
    expect(screen.getByTestId("sidebar-toggle-btn")).toBeInTheDocument();
  });

  it("renders the main panel with editor, preview, and action bar", () => {
    render(<App />);
    expect(screen.getByTestId("main-panel")).toBeInTheDocument();
    expect(screen.getByLabelText("LaTeX editor")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "复制到 Word" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "复制 LaTeX" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeInTheDocument();
  });

  it("renders the history sidebar by default", () => {
    render(<App />);
    expect(screen.getByTestId("history-sidebar")).toBeInTheDocument();
  });

  it("toggles sidebar visibility when toggle button is clicked", () => {
    render(<App />);
    const toggleBtn = screen.getByTestId("sidebar-toggle-btn");

    expect(screen.getByTestId("history-sidebar")).toBeInTheDocument();

    fireEvent.click(toggleBtn);
    expect(screen.queryByTestId("history-sidebar")).not.toBeInTheDocument();

    fireEvent.click(toggleBtn);
    expect(screen.getByTestId("history-sidebar")).toBeInTheDocument();
  });

  it("shows capture overlay when capture button is clicked", () => {
    render(<App />);

    expect(screen.queryByTestId("capture-overlay")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("capture-btn"));

    expect(screen.getByTestId("capture-overlay")).toBeInTheDocument();
  });

  it("shows recognizing indicator when isRecognizing is true", () => {
    useFormulaStore.setState({ isRecognizing: true });

    render(<App />);
    expect(screen.getByTestId("recognizing-indicator")).toBeInTheDocument();
    expect(screen.getByText("正在识别公式...")).toBeInTheDocument();
  });

  it("shows cancel button during recognition (Req 9.4)", () => {
    useFormulaStore.setState({ isRecognizing: true });

    render(<App />);
    expect(screen.getByTestId("cancel-recognition-btn")).toBeInTheDocument();
  });

  it("resets state when cancel recognition is clicked (Req 9.4)", () => {
    useFormulaStore.setState({
      isRecognizing: true,
      currentLatex: "x^2",
    });

    render(<App />);
    fireEvent.click(screen.getByTestId("cancel-recognition-btn"));

    const state = useFormulaStore.getState();
    expect(state.isRecognizing).toBe(false);
    expect(state.currentLatex).toBe("");
  });

  it("shows error toast when error is set", async () => {
    render(<App />);

    // Set error after mount (searchHistory on mount clears error)
    await act(async () => {
      useFormulaStore.setState({ error: "测试错误消息" });
    });

    expect(screen.getByTestId("error-toast")).toBeInTheDocument();
    expect(screen.getByText("测试错误消息")).toBeInTheDocument();
  });

  it("dismisses error toast when close button is clicked", async () => {
    render(<App />);

    await act(async () => {
      useFormulaStore.setState({ error: "测试错误消息" });
    });

    fireEvent.click(screen.getByLabelText("关闭错误提示"));

    expect(screen.queryByTestId("error-toast")).not.toBeInTheDocument();
  });

  it("does not show error toast when no error", () => {
    render(<App />);
    expect(screen.queryByTestId("error-toast")).not.toBeInTheDocument();
  });

  it("shows empty history message when no records", () => {
    render(<App />);
    expect(screen.getByTestId("history-empty")).toBeInTheDocument();
  });

  it("shows export error when no history records to export", () => {
    useFormulaStore.setState({ historyRecords: [] });

    render(<App />);
    fireEvent.click(screen.getByTestId("export-tex-btn"));

    const state = useFormulaStore.getState();
    expect(state.error).toBe("没有可导出的历史记录");
  });
});
