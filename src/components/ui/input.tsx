import type { InputHTMLAttributes } from "react";

import { cn } from "@/lib/cn";

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        "flex h-9 w-full rounded-xl border border-border/85 bg-white/86 px-3.5 text-[13px] text-foreground outline-none transition-all placeholder:text-muted-foreground focus:border-accent/55 focus:bg-white focus:ring-2 focus:ring-accent/14",
        className,
      )}
      {...props}
    />
  );
}
