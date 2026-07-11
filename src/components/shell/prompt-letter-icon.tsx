import { cn } from "@/lib/cn";

type PromptLetterIconProps = {
  className?: string;
};

export function PromptLetterIcon({ className }: PromptLetterIconProps) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-flex h-[22px] w-[22px] items-center justify-center font-sans text-[20px] font-bold leading-none tracking-[-0.04em]",
        className,
      )}
    >
      P
    </span>
  );
}
