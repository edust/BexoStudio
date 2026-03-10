import type { Transition } from "motion/react";

export const reorderLayoutTransition = {
  type: "spring",
  stiffness: 520,
  damping: 38,
  mass: 0.72,
} satisfies Transition;
