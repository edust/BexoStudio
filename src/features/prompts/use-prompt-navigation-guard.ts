import { App } from "antd";
import { useCallback, useEffect, useRef } from "react";
import { useBlocker, type BlockerFunction } from "react-router-dom";

export function usePromptNavigationGuard(dirty: boolean) {
  const { modal } = App.useApp();
  const blockerDialogOpenRef = useRef(false);

  useEffect(() => {
    if (!dirty) {
      return;
    }
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [dirty]);

  const shouldBlockNavigation = useCallback<BlockerFunction>(
    ({ currentLocation, nextLocation }) =>
      dirty && currentLocation.pathname !== nextLocation.pathname,
    [dirty],
  );
  const blocker = useBlocker(shouldBlockNavigation);

  useEffect(() => {
    if (blocker.state !== "blocked" || blockerDialogOpenRef.current) {
      return;
    }
    blockerDialogOpenRef.current = true;
    modal.confirm({
      title: "放弃未保存的 Prompt 修改？",
      content: "离开当前页面后，这些未保存内容将丢失。",
      okText: "放弃并离开",
      cancelText: "继续编辑",
      okButtonProps: { danger: true },
      onOk: () => blocker.proceed(),
      onCancel: () => blocker.reset(),
      afterClose: () => {
        blockerDialogOpenRef.current = false;
      },
    });
  }, [blocker, modal]);
}

