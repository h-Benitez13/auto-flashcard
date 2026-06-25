import { describe, it, expect, beforeEach } from "vitest";
import { useUiStore } from "./store";

/**
 * Zustand store tests
 *
 * Tests the UI state management layer.
 * No React rendering needed—Zustand stores can be tested directly.
 */
describe("useUiStore", () => {
  // Reset store to default state before each test
  const resetStore = () => {
    useUiStore.setState({
      viewMode: "study",
      gridColumns: 1,
      isTrashOpen: false,
    });
  };

  beforeEach(() => {
    resetStore();
  });

  it("should default to study view mode", () => {
    expect(useUiStore.getState().viewMode).toBe("study");
  });

  it("should switch view mode", () => {
    useUiStore.getState().setViewMode("grid");
    expect(useUiStore.getState().viewMode).toBe("grid");
  });

  it("should update grid columns", () => {
    useUiStore.getState().setGridColumns(3);
    expect(useUiStore.getState().gridColumns).toBe(3);
  });

  it("should toggle trash visibility", () => {
    expect(useUiStore.getState().isTrashOpen).toBe(false);

    useUiStore.getState().toggleTrash();
    expect(useUiStore.getState().isTrashOpen).toBe(true);

    useUiStore.getState().toggleTrash();
    expect(useUiStore.getState().isTrashOpen).toBe(false);
  });

  it("should set trash visibility directly", () => {
    useUiStore.getState().setIsTrashOpen(true);
    expect(useUiStore.getState().isTrashOpen).toBe(true);
  });
});
