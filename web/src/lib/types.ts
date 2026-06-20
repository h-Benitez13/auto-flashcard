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
