export interface PageContent {
  page_num: number;
  text: string;
  char_offset: number;
}

export interface DocumentInfo {
  id: string;
  filename: string;
  file_type: string;
  page_count: number;
  total_chars: number;
  pages: PageContent[];
}

export interface SourceRef {
  page_start: number;
  page_end: number;
  char_start: number;
  char_end: number;
  preview: string;
}

export interface Flashcard {
  id: string;
  document_id: string;
  chunk_id: string;
  question: string;
  answer: string;
  card_type: string;
  source_ref: SourceRef;
  tags: string[];
}

export interface GenerationJob {
  id: string;
  document_id: string;
  status: string;
  progress: number;
  total: number;
  error_message?: string;
  density?: string;
  use_llm: boolean;
}
