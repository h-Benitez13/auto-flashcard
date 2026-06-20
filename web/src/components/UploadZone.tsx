"use client";

import { useRef, useState } from "react";

interface Props {
  onUpload: (file: File) => Promise<void>;
  uploading: boolean;
}

export default function UploadZone({ onUpload, uploading }: Props) {
  const [drag, setDrag] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleFiles = (files: FileList | null) => {
    const file = files?.[0];
    if (!file) return;
    onUpload(file);
  };

  return (
    <div
      onClick={() => inputRef.current?.click()}
      onDragOver={(e) => {
        e.preventDefault();
        setDrag(true);
      }}
      onDragLeave={() => setDrag(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDrag(false);
        handleFiles(e.dataTransfer.files);
      }}
      className={`cursor-pointer rounded-2xl border-2 border-dashed p-8 text-center transition ${
        drag
          ? "border-sky-500 bg-sky-50"
          : "border-zinc-300 hover:border-zinc-400"
      }`}
    >
      <input
        ref={inputRef}
        type="file"
        className="hidden"
        accept=".pdf,.md,.txt"
        onChange={(e) => handleFiles(e.target.files)}
      />
      <p className="text-lg font-medium">
        {uploading ? "Uploading & parsing…" : "Drop a PDF or Markdown file here"}
      </p>
      <p className="text-sm text-zinc-500">or click to browse</p>
    </div>
  );
}
