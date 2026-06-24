import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import FlashcardList from "./FlashcardList";
import { useUiStore } from "@/lib/store";
import { Flashcard } from "@/lib/types";

/**
 * FlashcardList component tests
 */
const mockCards: Flashcard[] = [
  {
    id: "c1",
    document_id: "doc1",
    chunk_id: "ch1",
    question: "What is the capital of France?",
    answer: "Paris",
    card_type: "concept",
    source_ref: {
      page_start: 1,
      page_end: 1,
      char_start: 0,
      char_end: 10,
      preview: "France is a country",
    },
    tags: [],
    provider: "llm",
  },
  {
    id: "c2",
    document_id: "doc1",
    chunk_id: "ch2",
    question: "What is 2+2?",
    answer: "4",
    card_type: "concept",
    source_ref: {
      page_start: 1,
      page_end: 1,
      char_start: 0,
      char_end: 10,
      preview: "Math basics",
    },
    tags: [],
    provider: "rule-based",
  },
];

describe("FlashcardList", () => {
  beforeEach(() => {
    // Reset Zustand store
    useUiStore.setState({
      viewMode: "study",
      gridColumns: 1,
      isTrashOpen: false,
    });
  });

  it("renders empty state when no cards", () => {
    render(<FlashcardList cards={[]} />);
    expect(screen.getByText("No flashcards yet.")).toBeInTheDocument();
  });

  it("renders study mode by default", () => {
    render(<FlashcardList cards={mockCards} />);
    expect(screen.getByText("Study")).toBeInTheDocument();
    expect(screen.getByText("What is the capital of France?")).toBeInTheDocument();
  });

  it("switches to grid view", () => {
    render(<FlashcardList cards={mockCards} />);

    const gridButton = screen.getByText("Grid");
    fireEvent.click(gridButton);

    // In grid view, both questions are visible
    expect(screen.getByText("What is the capital of France?")).toBeInTheDocument();
    expect(screen.getByText("What is 2+2?")).toBeInTheDocument();
  });

  it("shows provider badges", () => {
    render(<FlashcardList cards={mockCards} />);
    expect(screen.getByText("LLM")).toBeInTheDocument();
  });

  it("flips card in study mode on click", () => {
    render(<FlashcardList cards={mockCards} />);

    const question = screen.getByText("What is the capital of France?");
    fireEvent.click(question);

    expect(screen.getByText("Paris")).toBeInTheDocument();
  });

  it("navigates to next card in study mode", () => {
    render(<FlashcardList cards={mockCards} />);

    const nextButton = screen.getByText("Next");
    fireEvent.click(nextButton);

    expect(screen.getByText("What is 2+2?")).toBeInTheDocument();
  });
});
