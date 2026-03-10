import { AlertTriangle, RefreshCcw } from "lucide-react";
import { Component, type ErrorInfo, type PropsWithChildren, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

type ErrorBoundaryProps = PropsWithChildren<{
  fallback?: ReactNode;
}>;

type ErrorBoundaryState = {
  hasError: boolean;
  error?: Error;
};

export class AppErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  public constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  public static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("UI runtime error", {
      error,
      componentStack: errorInfo.componentStack,
    });
  }

  private readonly handleReload = () => {
    window.location.reload();
  };

  public render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    if (this.props.fallback) {
      return this.props.fallback;
    }

    return (
      <div className="min-h-screen bg-background px-6 py-8 text-foreground">
        <div className="mx-auto flex min-h-[calc(100vh-4rem)] max-w-3xl items-center justify-center">
          <Card className="w-full border-border/80 bg-panel/92 shadow-[0_28px_64px_-32px_rgba(58,88,122,0.24)] backdrop-blur-xl">
            <CardHeader className="space-y-4">
              <div className="flex size-14 items-center justify-center rounded-2xl border border-danger/30 bg-danger/10 text-danger">
                <AlertTriangle className="size-7" />
              </div>
              <div className="space-y-2">
                <CardTitle className="text-2xl">界面发生异常</CardTitle>
                <CardDescription className="text-sm leading-6 text-muted-foreground">
                  Bexo Studio 已阻止异常继续扩散。你可以刷新当前窗口重新载入壳层。
                </CardDescription>
              </div>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="rounded-2xl border border-border/80 bg-panel-elevated/92 p-4 text-sm text-muted-foreground">
                <div className="mb-2 font-medium text-foreground/80">错误摘要</div>
                <p className="break-words font-mono text-xs leading-6">
                  {this.state.error?.message ?? "Unknown rendering error"}
                </p>
              </div>
              <Button className="w-full sm:w-auto" onClick={this.handleReload}>
                <RefreshCcw className="size-4" />
                重新加载窗口
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }
}
