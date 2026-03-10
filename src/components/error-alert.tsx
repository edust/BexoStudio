import type { ReactNode } from "react";
import { AlertTriangle } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/cn";

type ErrorAlertProps = {
  title: string;
  description: string;
  detail?: ReactNode;
  className?: string;
};

export function ErrorAlert({ className, description, detail, title }: ErrorAlertProps) {
  return (
    <Card className={cn("border-danger/40 bg-danger/8", className)}>
      <CardContent className="flex gap-3 p-4">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl border border-danger/30 bg-danger/10 text-danger">
          <AlertTriangle className="size-4" />
        </div>
        <div className="space-y-1">
          <div className="text-sm font-semibold text-foreground">{title}</div>
          <div className="text-sm leading-6 text-muted-foreground">{description}</div>
          {detail ? <div className="text-xs text-muted-foreground/90">{detail}</div> : null}
        </div>
      </CardContent>
    </Card>
  );
}
