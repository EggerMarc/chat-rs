import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Wraps a code block so clicking anywhere on it copies `code` to the
 * clipboard. A copy icon fades in on hover to make the affordance obvious.
 */
export function CopyableCode({
  code,
  className,
  children,
}: {
  code: string;
  className?: string;
  children: React.ReactNode;
}) {
  const [copied, setCopied] = useState(false);

  const copy = () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) return;
    navigator.clipboard
      .writeText(code)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
  };

  return (
    <div
      className={cn("group/copy relative cursor-pointer", className)}
      onClick={copy}
      role="button"
      tabIndex={0}
      aria-label="Copy code to clipboard"
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          copy();
        }
      }}
    >
      <div className="pointer-events-none absolute right-3 top-3 z-10 flex items-center gap-1.5 border border-border bg-card px-2 py-1 opacity-0 transition-opacity group-hover/copy:opacity-100">
        {copied ? (
          <Check className="size-3.5 text-primary" />
        ) : (
          <Copy className="size-3.5 text-muted-foreground" />
        )}
        <span className="font-label text-[10px] uppercase tracking-[0.15em] text-muted-foreground">
          {copied ? "Copied" : "Copy"}
        </span>
      </div>
      {children}
    </div>
  );
}
