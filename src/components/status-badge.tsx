import { Badge } from "@/components/ui/badge";

type StatusBadgeProps = {
  tone?: "default" | "success" | "warning" | "danger";
  children: string;
};

export function StatusBadge({ children, tone = "default" }: StatusBadgeProps) {
  const variant =
    tone === "success"
      ? "success"
      : tone === "warning"
        ? "warning"
        : tone === "danger"
          ? "danger"
          : "default";

  return <Badge variant={variant}>{children}</Badge>;
}
