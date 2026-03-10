import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";

import { cn } from "@/lib/cn";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-xl text-[13px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/55 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-accent text-accent-foreground shadow-[0_12px_24px_-18px_rgba(16,110,224,0.62)] hover:bg-accent/92",
        secondary:
          "border border-border/85 bg-panel-elevated/96 text-foreground shadow-[0_12px_28px_-22px_rgba(58,88,122,0.14)] hover:border-border hover:bg-white",
        ghost: "text-muted-foreground hover:bg-accent/6 hover:text-foreground",
        danger: "bg-danger text-white shadow-[0_12px_24px_-18px_rgba(194,67,34,0.48)] hover:bg-danger/90",
      },
      size: {
        default: "h-9 px-3.5 py-2",
        sm: "h-8 rounded-lg px-3 text-[12px]",
        lg: "h-10 rounded-xl px-4 text-[13px]",
        icon: "size-9 rounded-xl",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  };

export function Button({
  asChild = false,
  className,
  size,
  variant,
  ...props
}: ButtonProps) {
  const Component = asChild ? Slot : "button";
  return <Component className={cn(buttonVariants({ className, size, variant }))} {...props} />;
}
