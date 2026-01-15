import { useState, useRef, useEffect, useCallback } from "react";
import { Square, Image, ArrowUp, ChevronDown, X, FileCode2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { useSessionStore } from "../../stores/sessionStore";
import * as api from "../../api/client";
import { cn } from "../../lib/utils";

interface Attachment {
  path: string;
  name: string;
  type: "file" | "image";
  content?: string;
}

interface ProjectFile {
  path: string;
  name: string;
  is_dir: boolean;
}

export function ChatInput() {
  const [input, setInput] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [showMentions, setShowMentions] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionFiles, setMentionFiles] = useState<ProjectFile[]>([]);
  const [selectedMentionIndex, setSelectedMentionIndex] = useState(0);
  const [mentionStartPos, setMentionStartPos] = useState<number | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const mentionRef = useRef<HTMLDivElement>(null);

  const sendMessage = useSessionStore((s) => s.sendMessage);
  const cancelOperation = useSessionStore((s) => s.cancelOperation);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const agentMode = useSessionStore((s) => s.agentMode);
  const setAgentMode = useSessionStore((s) => s.setAgentMode);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(
        textareaRef.current.scrollHeight,
        200
      )}px`;
    }
  }, [input]);

  // Search for files when mention query changes
  useEffect(() => {
    if (!showMentions || !activeSessionId || !mentionQuery) {
      setMentionFiles([]);
      return;
    }

    const searchFiles = async () => {
      try {
        const response = await api.listProjectFiles(activeSessionId, mentionQuery, 10);
        setMentionFiles(response.files);
        setSelectedMentionIndex(0);
      } catch (error) {
        console.error("Failed to search files:", error);
        setMentionFiles([]);
      }
    };

    const debounce = setTimeout(searchFiles, 150);
    return () => clearTimeout(debounce);
  }, [showMentions, mentionQuery, activeSessionId]);

  // Handle image attachment
  const handleImageAttach = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Images",
            extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"],
          },
        ],
        title: "Select Images",
      });

      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        const newAttachments: Attachment[] = [];

        for (const path of paths) {
          const name = path.split("/").pop() || path;
          try {
            // Read image and convert to base64
            const fileData = await readFile(path);
            const base64 = btoa(
              Array.from(fileData)
                .map((byte) => String.fromCharCode(byte))
                .join("")
            );
            const ext = name.split(".").pop()?.toLowerCase() || "png";
            const mimeType = ext === "jpg" ? "jpeg" : ext;

            newAttachments.push({
              path,
              name,
              type: "image",
              content: `data:image/${mimeType};base64,${base64}`,
            });
          } catch (err) {
            console.error("Failed to read image:", err);
          }
        }

        setAttachments((prev) => [...prev, ...newAttachments]);
      }
    } catch (error) {
      console.error("Failed to open image picker:", error);
    }
  };

  // Insert mention into input
  const insertMention = useCallback(
    (file: ProjectFile) => {
      if (mentionStartPos === null) return;

      const before = input.slice(0, mentionStartPos);
      const after = input.slice(textareaRef.current?.selectionStart || input.length);

      // Add file as attachment
      setAttachments((prev) => [
        ...prev.filter((a) => a.path !== file.path),
        { path: file.path, name: file.name, type: "file" },
      ]);

      // Insert @filename in text
      setInput(before + `@${file.name} ` + after);
      setShowMentions(false);
      setMentionQuery("");
      setMentionStartPos(null);

      // Focus textarea
      setTimeout(() => textareaRef.current?.focus(), 0);
    },
    [input, mentionStartPos]
  );

  // Remove attachment
  const removeAttachment = (path: string) => {
    setAttachments((prev) => prev.filter((a) => a.path !== path));
  };

  const handleSubmit = async () => {
    if ((!input.trim() && attachments.length === 0) || isProcessing) return;

    const message = input.trim();
    setInput("");
    setAttachments([]);

    // Send message (attachments to be implemented later)
    await sendMessage(message);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Handle mention navigation
    if (showMentions && mentionFiles.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedMentionIndex((prev) =>
          prev < mentionFiles.length - 1 ? prev + 1 : 0
        );
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedMentionIndex((prev) =>
          prev > 0 ? prev - 1 : mentionFiles.length - 1
        );
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        insertMention(mentionFiles[selectedMentionIndex]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setShowMentions(false);
        setMentionQuery("");
        setMentionStartPos(null);
        return;
      }
    }

    // Normal enter to send
    if (e.key === "Enter" && !e.shiftKey && !showMentions) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    const cursorPos = e.target.selectionStart;
    setInput(value);

    // Check for @ mention trigger
    const textBeforeCursor = value.slice(0, cursorPos);
    const lastAtIndex = textBeforeCursor.lastIndexOf("@");

    if (lastAtIndex !== -1) {
      const textAfterAt = textBeforeCursor.slice(lastAtIndex + 1);
      // Only trigger if @ is at start or after whitespace, and no space after @
      const charBeforeAt = lastAtIndex > 0 ? value[lastAtIndex - 1] : " ";
      if ((charBeforeAt === " " || charBeforeAt === "\n" || lastAtIndex === 0) && !textAfterAt.includes(" ")) {
        setShowMentions(true);
        setMentionQuery(textAfterAt);
        setMentionStartPos(lastAtIndex);
        return;
      }
    }

    setShowMentions(false);
    setMentionQuery("");
    setMentionStartPos(null);
  };

  return (
    <div className="border-t border-border bg-card/50 p-4">
      <div className="max-w-4xl mx-auto">
        {/* Attachments preview */}
        {attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 mb-3">
            {attachments.map((attachment) => (
              <div
                key={attachment.path}
                className="flex items-center gap-2 px-2.5 py-1.5 bg-muted/50 border border-border rounded-lg text-xs"
              >
                {attachment.type === "image" ? (
                  <Image className="w-3.5 h-3.5 text-blue-400" />
                ) : (
                  <FileCode2 className="w-3.5 h-3.5 text-primary" />
                )}
                <span className="text-foreground max-w-[150px] truncate">
                  {attachment.name}
                </span>
                <button
                  onClick={() => removeAttachment(attachment.path)}
                  className="p-0.5 hover:bg-muted rounded transition-colors"
                >
                  <X className="w-3 h-3 text-muted-foreground hover:text-foreground" />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Input container - OpenCode style floating card */}
        <div className="relative bg-muted/50 border border-border rounded-xl overflow-visible">
          {/* @ Mention dropdown */}
          {showMentions && mentionFiles.length > 0 && (
            <div
              ref={mentionRef}
              className="absolute bottom-full left-0 right-0 mb-2 bg-card border border-border rounded-lg shadow-lg overflow-hidden z-50"
            >
              <div className="px-3 py-2 border-b border-border">
                <span className="text-xs text-muted-foreground">
                  Files matching "{mentionQuery}"
                </span>
              </div>
              <div className="max-h-48 overflow-y-auto">
                {mentionFiles.map((file, index) => (
                  <button
                    key={file.path}
                    onClick={() => insertMention(file)}
                    className={cn(
                      "w-full flex items-center gap-2 px-3 py-2 text-left transition-colors",
                      index === selectedMentionIndex
                        ? "bg-primary/10 text-primary"
                        : "hover:bg-muted text-foreground"
                    )}
                  >
                    <FileCode2 className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{file.name}</div>
                      <div className="text-xs text-muted-foreground truncate">
                        {file.path}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
              <div className="px-3 py-1.5 border-t border-border bg-muted/30">
                <span className="text-[10px] text-muted-foreground">
                  ↑↓ navigate • Enter select • Esc close
                </span>
              </div>
            </div>
          )}

          {/* Textarea */}
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            placeholder='Ask anything... Use @ to mention files'
            disabled={isProcessing}
            rows={1}
            className="w-full bg-transparent text-sm text-foreground placeholder:text-muted-foreground resize-none focus:outline-none min-h-[48px] max-h-[200px] p-4 pb-14"
          />

          {/* Bottom toolbar */}
          <div className="absolute bottom-0 left-0 right-0 flex items-center justify-between px-3 py-2 bg-muted/30">
            <div className="flex items-center gap-2">
              {/* Mode selector - dropdown style */}
              <button
                onClick={() => setAgentMode(agentMode === "build" ? "plan" : "build")}
                disabled={isProcessing}
                className={cn(
                  "flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium rounded-md transition-all",
                  agentMode === "build"
                    ? "bg-primary/15 text-primary border border-primary/30 hover:bg-primary/25"
                    : "bg-amber-500/15 text-amber-400 border border-amber-500/30 hover:bg-amber-500/25"
                )}
              >
                {agentMode === "build" ? "Build" : "Plan"}
                <ChevronDown className="w-3 h-3" />
              </button>

              {/* Model indicator */}
              <div className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-muted-foreground rounded-md hover:bg-muted/50 cursor-pointer transition-colors">
                <span className="w-1.5 h-1.5 rounded-full bg-primary" />
                <span>Copilot</span>
                <ChevronDown className="w-3 h-3" />
              </div>
            </div>

            <div className="flex items-center gap-2">
              {/* Image attachment */}
              <button
                onClick={handleImageAttach}
                disabled={isProcessing}
                className="p-2 text-muted-foreground hover:text-foreground hover:bg-muted rounded-md transition-colors"
                title="Attach image"
              >
                <Image className="w-4 h-4" />
              </button>

              {/* Send/Cancel button */}
              {isProcessing ? (
                <button
                  onClick={cancelOperation}
                  className="p-2 bg-destructive text-destructive-foreground rounded-md hover:bg-destructive/90 transition-colors"
                  title="Cancel (Esc)"
                >
                  <Square className="w-4 h-4" />
                </button>
              ) : (
                <button
                  onClick={handleSubmit}
                  disabled={!input.trim() && attachments.length === 0}
                  className={cn(
                    "p-2 rounded-md transition-colors",
                    input.trim() || attachments.length > 0
                      ? "bg-primary text-primary-foreground hover:bg-primary/90"
                      : "bg-muted text-muted-foreground cursor-not-allowed"
                  )}
                  title="Send (Enter)"
                >
                  <ArrowUp className="w-4 h-4" />
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Hints */}
        <div className="flex items-center justify-between mt-2 text-[10px] text-muted-foreground">
          <span>Enter to send • Shift+Enter for new line • @ to mention files</span>
        </div>
      </div>
    </div>
  );
}
