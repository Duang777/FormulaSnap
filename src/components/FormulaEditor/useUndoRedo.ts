// useUndoRedo - Custom hook for undo/redo functionality
// Maintains an explicit history stack for reliable undo/redo
// Validates: Requirements 4.3

import { useCallback, useRef } from "react";

export interface UndoRedoState {
  /** Push a new state onto the history stack */
  pushState: (value: string) => void;
  /** Undo to the previous state */
  undo: () => void;
  /** Redo to the next state */
  redo: () => void;
  /** Whether undo is available */
  canUndo: () => boolean;
  /** Whether redo is available */
  canRedo: () => boolean;
}

const MAX_HISTORY_SIZE = 100;

/**
 * Custom hook that provides undo/redo functionality for a text value.
 *
 * @param currentValue - The current value (controlled externally)
 * @param onChange - Callback to update the external value
 * @returns UndoRedoState with push, undo, redo operations
 */
export function useUndoRedo(
  currentValue: string,
  onChange: (value: string) => void
): UndoRedoState {
  // History stack: past states (not including current)
  const pastRef = useRef<string[]>([]);
  // Future stack: states that were undone
  const futureRef = useRef<string[]>([]);

  const pushState = useCallback(
    (_newValue: string) => {
      // Push current value to past stack
      pastRef.current = [...pastRef.current, currentValue];
      // Clear future on new input (standard undo/redo behavior)
      futureRef.current = [];

      // Limit history size
      if (pastRef.current.length > MAX_HISTORY_SIZE) {
        pastRef.current = pastRef.current.slice(
          pastRef.current.length - MAX_HISTORY_SIZE
        );
      }
    },
    [currentValue]
  );

  const undo = useCallback(() => {
    if (pastRef.current.length === 0) return;

    const past = [...pastRef.current];
    const previousValue = past.pop()!;

    pastRef.current = past;
    // Push current value to future stack for redo
    futureRef.current = [...futureRef.current, currentValue];

    onChange(previousValue);
  }, [currentValue, onChange]);

  const redo = useCallback(() => {
    if (futureRef.current.length === 0) return;

    const future = [...futureRef.current];
    const nextValue = future.pop()!;

    futureRef.current = future;
    // Push current value to past stack
    pastRef.current = [...pastRef.current, currentValue];

    onChange(nextValue);
  }, [currentValue, onChange]);

  const canUndo = useCallback(() => pastRef.current.length > 0, []);
  const canRedo = useCallback(() => futureRef.current.length > 0, []);

  return { pushState, undo, redo, canUndo, canRedo };
}
