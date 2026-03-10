import type { LucideIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

type EmptyStateProps = {
  icon: LucideIcon;
  title: string;
  description: string;
  aside?: string;
};

export function EmptyState({ aside, description, icon: Icon, title }: EmptyStateProps) {
  return (
    <Card className="overflow-hidden">
      <CardHeader className="relative border-b border-border/60">
        <div className="absolute inset-x-0 top-0 h-16 bg-[radial-gradient(circle_at_top_left,rgba(0,197,255,0.14),transparent_58%)]" />
        <div className="relative flex size-11 items-center justify-center rounded-xl border border-accent/20 bg-accent/10 text-accent">
          <Icon className="size-5" />
        </div>
        <CardTitle className="relative mt-3 text-lg">{title}</CardTitle>
        <CardDescription className="relative max-w-xl">{description}</CardDescription>
      </CardHeader>
      {aside ? (
        <CardContent className="pt-5 text-[13px] leading-5 text-muted-foreground">{aside}</CardContent>
      ) : null}
    </Card>
  );
}
