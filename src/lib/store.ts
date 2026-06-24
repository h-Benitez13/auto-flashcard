/**
 * ZUSTAND STORE - UI State Management
 *
 * WHAT IS ZUSTAND?
 * Zustand is a lightweight state management library (~2KB).
 * It's like Context + useReducer, but simpler and with less boilerplate.
 *
 * KEY DIFFERENCES: Zustand vs Jotai vs React Context
 *
 * 1. REACT CONTEXT (Built-in)
 * - Free (no dependency)
 * - Causes re-render of entire tree if state changes
 * - Requires wrapping Provider + Consumer pattern
 * - Good for: app theme, auth (rarely changes)
 * - Bad for: frequently changing UI state (causes perf issues)
 *
 * Example:
 *   const ThemeContext = createContext();
 *   <ThemeContext.Provider value={{ theme: 'dark' }}>
 *     <App />
 *   </ThemeContext.Provider>
 *   // Note: entire tree re-renders on theme change
 *
 * 2. JOTAI (Atom-based)
 * - Atoms are like tiny, isolated pieces of state
 * - Only components using a specific atom re-render
 * - Very granular, very flexible
 * - Good for: complex state with lots of derived values
 * - Bad for: simple UI state (overkill), learning curve
 *
 * Example:
 *   const viewModeAtom = atom<'study' | 'grid'>('study');
 *   const gridColumnsAtom = atom<1 | 2 | 3>(1);
 *
 *   function Component() {
 *     const [viewMode, setViewMode] = useAtom(viewModeAtom);
 *     // Only this component re-renders on viewMode change
 *   }
 *
 * 3. ZUSTAND (Store-based) <- We're using this
 * - Single store (like Redux) but much simpler
 * - Components subscribe only to fields they use
 * - No Provider needed (just create hook)
 * - Good for: simple UI state, easy migration from Context
 * - Bad for: very granular state (Jotai is better)
 *
 * Example (this file):
 *   const useUiStore = create((set) => ({
 *     viewMode: 'study',
 *     setViewMode: (mode) => set({ viewMode: mode }),
 *   }));
 *
 *   function Component() {
 *     const viewMode = useUiStore((state) => state.viewMode);
 *     // Only this component re-renders on viewMode change (via selector)
 *   }
 *
 * WHY ZUSTAND FOR THIS APP?
 * - UI state (view mode, grid columns, trash open/closed) is simple and infrequent
 * - No Provider wrapping needed (cleaner)
 * - Easier than Context (no useReducer boilerplate)
 * - Less granular than Jotai (which is overkill here)
 * - Perfect for: "study view vs grid view" toggle state
 *
 * WHY NOT ZUSTAND FOR DATA?
 * - TanStack Query owns document/flashcard/job data
 * - Zustand doesn't handle caching, deduplication, etc.
 * - TanStack Query is purpose-built for server state
 *
 * RULE OF THUMB:
 * - Zustand: UI state (what the user is viewing, how)
 * - TanStack Query: Data state (documents, flashcards)
 */

import { create } from "zustand";

/**
 * UI State Store
 *
 * This store manages:
 * 1. viewMode: "study" vs "grid" view for flashcards
 * 2. gridColumns: 1, 2, or 3 column layout
 * 3. isTrashOpen: whether trash section is expanded
 *
 * NOTE: Document/flashcard data is managed by TanStack Query,
 *       NOT by this store. See hooks/useDocuments.ts for data.
 */

interface UiState {
  // Flashcard viewing mode
  viewMode: "study" | "grid";
  setViewMode: (mode: "study" | "grid") => void;

  // Grid layout columns (only used in "grid" mode)
  gridColumns: 1 | 2 | 3;
  setGridColumns: (cols: 1 | 2 | 3) => void;

  // Whether trash section is expanded on home page
  isTrashOpen: boolean;
  setIsTrashOpen: (open: boolean) => void;

  // Toggle trash visibility
  toggleTrash: () => void;
}

/**
 * How Zustand works (vs Jotai):
 *
 * ZUSTAND:
 *   const store = create((set, get) => ({
 *     count: 0,
 *     increment: () => set({ count: get().count + 1 }),
 *   }));
 *
 *   In component:
 *     const count = store((state) => state.count);
 *     // selector syntax returns only `count` field
 *     // component only re-renders if `count` changes
 *
 * JOTAI (for comparison):
 *   const countAtom = atom(0);
 *   const incrementAtom = atom(null, (get, set) => {
 *     set(countAtom, get(countAtom) + 1);
 *   });
 *
 *   In component:
 *     const [count, increment] = useAtom(countAtom);
 *     // Each atom is independent, more fine-grained
 *
 * CONTEXT (for comparison):
 *   const CountContext = createContext();
 *   <CountContext.Provider value={{ count, increment }}>
 *     <App />
 *   </CountContext.Provider>
 *   // Entire tree re-renders on any state change
 *
 * KEY DIFFERENCE:
 * - Zustand: Selector pattern -> fine-grained updates
 * - Jotai: Each atom independent -> very fine-grained
 * - Context: All-or-nothing -> full tree re-render
 */

export const useUiStore = create<UiState>((set) => ({
  // Initial state
  viewMode: "study",

  // Action: change view mode
  // Usage in component:
  //   const setViewMode = useUiStore((state) => state.setViewMode);
  //   setViewMode("grid");
  setViewMode: (mode) => set({ viewMode: mode }),

  // Initial state
  gridColumns: 1,

  // Action: change grid columns
  setGridColumns: (cols) => set({ gridColumns: cols }),

  // Initial state
  isTrashOpen: false,

  // Action: set trash visibility
  setIsTrashOpen: (open) => set({ isTrashOpen: open }),

  // Action: toggle trash visibility
  // Usage:
  //   const toggleTrash = useUiStore((state) => state.toggleTrash);
  //   <button onClick={toggleTrash}>
  toggleTrash: () => set((state) => ({ isTrashOpen: !state.isTrashOpen })),
}));

/**
 * SELECTOR PATTERN (How to use in components):
 *
 * CORRECT (only re-render on viewMode change):
 *   const viewMode = useUiStore((state) => state.viewMode);
 *   const setViewMode = useUiStore((state) => state.setViewMode);
 *
 * WRONG (re-renders on any state change):
 *   const { viewMode, setViewMode } = useUiStore();
 *   // This subscribes to entire store, not just viewMode
 *
 * TIP: Always use inline selectors:
 *   const viewMode = useUiStore((state) => state.viewMode);
 *   const setViewMode = useUiStore((state) => state.setViewMode);
 */

/**
 * MIGRATION PATH if you want Jotai later:
 *
 * If Zustand becomes too simple and you need more complex state,
 * it's easy to migrate to Jotai:
 *
 * Just replace this file with:
 *
 *   import { atom } from 'jotai';
 *
 *   export const viewModeAtom = atom<'study' | 'grid'>('study');
 *   export const gridColumnsAtom = atom<1 | 2 | 3>(1);
 *   export const isTrashOpenAtom = atom(false);
 *
 * Then in components, change:
 *   const viewMode = useUiStore((s) => s.viewMode);
 *   to
 *   const [viewMode] = useAtom(viewModeAtom);
 *
 * Same API shape, just different implementation.
 */
