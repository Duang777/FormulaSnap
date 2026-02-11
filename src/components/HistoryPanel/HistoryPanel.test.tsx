import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { HistoryRecord } from "../../types";
import {
  renderLatexToHtml,
  formatTimestamp,
  formatConfidence,
  HistoryRecordItem,
} from "./HistoryPanel";

// ============================================================
// Mock the zustand store
// ============================================================

const mockSearchHistory = vi.fn();
const mockToggleFavorite = vi.fn();
const mockSetSearchQuery = vi.fn();

let mockStoreState: Record<string, unknown> = {
  historyRecords: [] as HistoryRecord[],
  searchQuery: "",
  searchHistory: mockSearchHistory,
  toggleFavorite: mockToggleFavorite,
  setSearchQuery: mockSetSearchQuery,
};

vi.mock("../../store/formulaStore", () => ({
  useFormulaStore: (selector?: (state: Record<string, unknown>) => unknown) => {
    if (selector) {
      return selector(mockStoreState);
    }
    return mockStoreState;
  },
}));

// ============================================================
// Test data helpers
// ============================================================

function makeRecord(overrides: Partial<HistoryRecord> = {}): HistoryRecord {
  return {
    id: 1,
    created_at: "2024-01-15T10:30:00.000Z",
    original_latex: "x^2 + y^2 = z^2",
    confidence: 0.95,
    engine_version: "pix2tex-onnx-1.0",
    is_favorite: false,
    ...overrides,
  };
}

// ============================================================
// renderLatexToHtml unit tests
// ============================================================

describe("renderLatexToHtml", () => {
  it("renders valid LaTeX to HTML", () => {
    const result = renderLatexToHtml("x^2");
    expect(result.html).toBeTruthy();
    expect(result.error).toBeNull();
    expect(result.html).toContain("katex");
  });

  it("returns error for invalid LaTeX", () => {
    const result = renderLatexToHtml("\\frac{a}{");
    expect(result.error).toBeTruthy();
    expect(result.html).toBe("");
  });

  it("returns empty html and no error for empty string", () => {
    const result = renderLatexToHtml("");
    expect(result.html).toBe("");
    expect(result.error).toBeNull();
  });

  it("returns empty html and no error for whitespace-only string", () => {
    const result = renderLatexToHtml("   ");
    expect(result.html).toBe("");
    expect(result.error).toBeNull();
  });
});

// ============================================================
// formatTimestamp unit tests
// ============================================================

describe("formatTimestamp", () => {
  it("formats a valid ISO string", () => {
    const result = formatTimestamp("2024-01-15T10:30:00.000Z");
    expect(result).toBeTruthy();
    expect(result).not.toBe("");
  });

  it("returns the original string for invalid dates", () => {
    const result = formatTimestamp("not-a-date");
    expect(result).toBe("not-a-date");
  });
});

// ============================================================
// formatConfidence unit tests
// ============================================================

describe("formatConfidence", () => {
  it("formats 0.95 as 95%", () => {
    expect(formatConfidence(0.95)).toBe("95%");
  });

  it("formats 1.0 as 100%", () => {
    expect(formatConfidence(1.0)).toBe("100%");
  });

  it("formats 0 as 0%", () => {
    expect(formatConfidence(0)).toBe("0%");
  });

  it("rounds 0.956 to 96%", () => {
    expect(formatConfidence(0.956)).toBe("96%");
  });
});

// ============================================================
// HistoryPanel component tests
// ============================================================

describe("HistoryPanel", () => {
  // Lazy import to use after mock is set up
  let HistoryPanel: typeof import("./HistoryPanel").HistoryPanel;

  beforeEach(async () => {
    vi.clearAllMocks();
    mockStoreState = {
      historyRecords: [] as HistoryRecord[],
      searchQuery: "",
      searchHistory: mockSearchHistory,
      toggleFavorite: mockToggleFavorite,
      setSearchQuery: mockSetSearchQuery,
    };
    const mod = await import("./HistoryPanel");
    HistoryPanel = mod.HistoryPanel;
  });

  const defaultProps = {
    onSelect: vi.fn(),
    onCopyToWord: vi.fn(),
    onCopyLatex: vi.fn(),
  };

  it("renders the history panel with search input", () => {
    render(<HistoryPanel {...defaultProps} />);
    expect(screen.getByTestId("history-panel")).toBeInTheDocument();
    expect(screen.getByTestId("history-search-input")).toBeInTheDocument();
  });

  it("shows empty state when no records", () => {
    render(<HistoryPanel {...defaultProps} />);
    expect(screen.getByTestId("history-empty")).toBeInTheDocument();
    expect(screen.getByText("暂无历史记录")).toBeInTheDocument();
  });

  it("renders the search input with correct placeholder", () => {
    render(<HistoryPanel {...defaultProps} />);
    const input = screen.getByTestId("history-search-input");
    expect(input).toHaveAttribute("placeholder", "搜索...");
  });

  it("search input has correct aria-label", () => {
    render(<HistoryPanel {...defaultProps} />);
    expect(
      screen.getByRole("textbox", { name: "搜索历史记录" })
    ).toBeInTheDocument();
  });

  it("calls searchHistory when typing in search box (Req 7.2)", () => {
    render(<HistoryPanel {...defaultProps} />);
    const input = screen.getByTestId("history-search-input");
    fireEvent.change(input, { target: { value: "frac" } });
    expect(mockSetSearchQuery).toHaveBeenCalledWith("frac");
    expect(mockSearchHistory).toHaveBeenCalledWith("frac");
  });

  it("renders records from the store", () => {
    const records = [
      makeRecord({ id: 1, original_latex: "x^2" }),
      makeRecord({ id: 2, original_latex: "y^2" }),
    ];
    mockStoreState.historyRecords = records;

    render(<HistoryPanel {...defaultProps} />);
    const recordElements = screen.getAllByTestId("history-record");
    expect(recordElements.length).toBe(2);
  });

  it("shows 'no matching records' when search has no results", () => {
    mockStoreState.searchQuery = "nonexistent";
    mockStoreState.historyRecords = [];

    render(<HistoryPanel {...defaultProps} />);
    expect(screen.getByText("未找到匹配记录")).toBeInTheDocument();
  });

  it("calls onCopyToWord when copy-to-word button is clicked (Req 7.4)", () => {
    const onCopyToWord = vi.fn();
    const records = [makeRecord()];
    mockStoreState.historyRecords = records;

    render(
      <HistoryPanel
        onSelect={vi.fn()}
        onCopyToWord={onCopyToWord}
        onCopyLatex={vi.fn()}
      />
    );

    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-copy-word-btn"));
    expect(onCopyToWord).toHaveBeenCalledWith(records[0]);
  });

  it("calls onCopyLatex when copy-latex button is clicked", () => {
    const onCopyLatex = vi.fn();
    const records = [makeRecord()];
    mockStoreState.historyRecords = records;

    render(
      <HistoryPanel
        onSelect={vi.fn()}
        onCopyToWord={vi.fn()}
        onCopyLatex={onCopyLatex}
      />
    );

    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-copy-latex-btn"));
    expect(onCopyLatex).toHaveBeenCalledWith(records[0]);
  });

  it("calls onSelect when edit button is clicked (Req 7.5)", () => {
    const onSelect = vi.fn();
    const records = [makeRecord()];
    mockStoreState.historyRecords = records;

    render(
      <HistoryPanel
        onSelect={onSelect}
        onCopyToWord={vi.fn()}
        onCopyLatex={vi.fn()}
      />
    );

    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-edit-btn"));
    expect(onSelect).toHaveBeenCalledWith(records[0]);
  });

  it("calls toggleFavorite when favorite button is clicked (Req 7.3)", () => {
    const records = [makeRecord({ id: 42 })];
    mockStoreState.historyRecords = records;

    render(<HistoryPanel {...defaultProps} />);

    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-favorite-btn"));
    expect(mockToggleFavorite).toHaveBeenCalledWith(42);
  });

  it("filters records client-side based on search query", () => {
    const records = [
      makeRecord({ id: 1, original_latex: "\\frac{a}{b}" }),
      makeRecord({ id: 2, original_latex: "x^2 + y^2" }),
    ];
    mockStoreState.historyRecords = records;
    mockStoreState.searchQuery = "frac";

    render(<HistoryPanel {...defaultProps} />);
    const recordElements = screen.getAllByTestId("history-record");
    expect(recordElements.length).toBe(1);
  });
});

// ============================================================
// HistoryRecordItem tests (direct rendering)
// ============================================================

describe("HistoryRecordItem", () => {
  const defaultItemProps = {
    onSelect: vi.fn(),
    onCopyToWord: vi.fn(),
    onCopyLatex: vi.fn(),
    onToggleFavorite: vi.fn(),
  };

  it("renders KaTeX preview for valid LaTeX on expand", () => {
    const record = makeRecord({ original_latex: "x^2" });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Preview is shown on click to expand
    const header = screen.getByTestId("history-record-header");
    fireEvent.click(header);
    expect(screen.getByTestId("history-preview")).toBeInTheDocument();
  });

  it("renders error preview for invalid LaTeX", () => {
    const record = makeRecord({ original_latex: "\\frac{a}{" });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Expand to see error
    const header = screen.getByTestId("history-record-header");
    fireEvent.click(header);
    expect(screen.getByTestId("history-preview-error")).toBeInTheDocument();
  });

  it("renders confidence percentage", () => {
    const record = makeRecord({ confidence: 0.87 });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    expect(screen.getByTestId("history-confidence")).toHaveTextContent(
      "87%"
    );
  });

  it("renders timestamp", () => {
    const record = makeRecord();
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    expect(screen.getByTestId("history-timestamp")).toBeInTheDocument();
  });

  it("shows unfavorited state correctly", () => {
    const record = makeRecord({ is_favorite: false });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    const favBtn = screen.getByTestId("history-favorite-btn");
    expect(favBtn).toHaveAttribute("aria-label", "收藏");
  });

  it("shows favorited state correctly", () => {
    const record = makeRecord({ is_favorite: true });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    const favBtn = screen.getByTestId("history-favorite-btn");
    expect(favBtn).toHaveAttribute("aria-label", "取消收藏");
  });

  it("renders all four action buttons", () => {
    const record = makeRecord();
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    expect(screen.getByTestId("history-copy-word-btn")).toBeInTheDocument();
    expect(screen.getByTestId("history-copy-latex-btn")).toBeInTheDocument();
    expect(screen.getByTestId("history-edit-btn")).toBeInTheDocument();
    expect(screen.getByTestId("history-favorite-btn")).toBeInTheDocument();
  });

  it("uses edited_latex for preview when available", () => {
    const record = makeRecord({
      original_latex: "\\frac{a}{",  // invalid
      edited_latex: "y^2",           // valid
    });
    render(<HistoryRecordItem record={record} {...defaultItemProps} />);
    // Expand to see preview
    const header = screen.getByTestId("history-record-header");
    fireEvent.click(header);
    // Should render successfully since edited_latex is valid
    expect(screen.getByTestId("history-preview")).toBeInTheDocument();
    expect(screen.queryByTestId("history-preview-error")).not.toBeInTheDocument();
  });

  it("calls onCopyToWord with the record", () => {
    const onCopyToWord = vi.fn();
    const record = makeRecord();
    render(
      <HistoryRecordItem
        record={record}
        {...defaultItemProps}
        onCopyToWord={onCopyToWord}
      />
    );
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-copy-word-btn"));
    expect(onCopyToWord).toHaveBeenCalledWith(record);
  });

  it("calls onCopyLatex with the record", () => {
    const onCopyLatex = vi.fn();
    const record = makeRecord();
    render(
      <HistoryRecordItem
        record={record}
        {...defaultItemProps}
        onCopyLatex={onCopyLatex}
      />
    );
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-copy-latex-btn"));
    expect(onCopyLatex).toHaveBeenCalledWith(record);
  });

  it("calls onSelect with the record when edit is clicked", () => {
    const onSelect = vi.fn();
    const record = makeRecord();
    render(
      <HistoryRecordItem
        record={record}
        {...defaultItemProps}
        onSelect={onSelect}
      />
    );
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-edit-btn"));
    expect(onSelect).toHaveBeenCalledWith(record);
  });

  it("calls onToggleFavorite with the record", () => {
    const onToggleFavorite = vi.fn();
    const record = makeRecord();
    render(
      <HistoryRecordItem
        record={record}
        {...defaultItemProps}
        onToggleFavorite={onToggleFavorite}
      />
    );
    // Expand first
    fireEvent.click(screen.getByTestId("history-record-header"));
    fireEvent.click(screen.getByTestId("history-favorite-btn"));
    expect(onToggleFavorite).toHaveBeenCalledWith(record);
  });
});
