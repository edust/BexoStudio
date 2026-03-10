import type { TextareaHTMLAttributes } from "react";

import { cn } from "@/lib/cn";

export function Textarea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={cn(
        "flex min-h-24 w-full rounded-xl border border-border/85 bg-white/86 px-3.5 py-3 text-[13px] text-foreground outline-none transition-all placeholder:text-muted-foreground focus:border-accent/55 focus:bg-white focus:ring-2 focus:ring-accent/14",
        className,
      )}
      {...props}
    />
  );
}
