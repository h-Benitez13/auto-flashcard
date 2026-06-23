"use client";

import { useRef, useState } from "react";
import { Upload } from "lucide-react";

import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

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
    <Card
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
      className={cn(
        "cursor-pointer border-dashed p-10 text-center transition hover:border-primary/50 hover:bg-accent",
        drag && "border-primary bg-accent"
      )}
    >
      <input
        ref={inputRef}
        type="file"
        className="hidden"
        accept=".pdf,.md,.txt,.pptx,.ppt"
        onChange={(e) => handleFiles(e.target.files)}
      />
      <div className="flex flex-col items-center gap-3">
        <div className="rounded-full bg-primary/10 p-3">
          <Upload className="size-6 text-primary" />
        </div>
        <p className="text-lg font-medium">
          {uploading ? "Uploading & parsing…" : "Drop a file to upload"}
        </p>
        <p className="text-sm text-muted-foreground">
          PDF, Markdown, or Text. Click to browse.
        </p>
      </div>
    </Card>
  );
}
