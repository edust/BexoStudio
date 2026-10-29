import { cn } from "@/lib/cn";

type PromptLetterIconProps = {
  className?: string;
};

export function PromptLetterIcon({ className }: PromptLetterIconProps) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-flex h-[18px] w-[18px] items-center justify-center font-sans text-[16px] font-semibold leading-none",
        className,
      )}
    >
      P
    </span>
  );
}
