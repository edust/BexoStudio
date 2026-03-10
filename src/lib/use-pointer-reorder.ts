import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

export type PointerReorderChange<T> = {
  draggingId: string;
  targetId: string;
  nextItems: T[];
};

type UsePointerReorderOptions<T> = {
  disabled?: boolean;
  items: T[];
  reorderItems: (items: T[], draggingId: string, targetId: string) => T[] | null;
  onReorder: (change: PointerReorderChange<T>) => void | Promise<void>;
};

type ActiveDragState = {
  draggingId: string;
  pointerId: number;
};

export function usePointerReorder<T>(options: UsePointerReorderOptions<T>) {
  const { disabled = false, items, reorderItems, onReorder } = options;
  const [displayItems, setDisplayItems] = useState(items);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  const activeDragRef = useRef<ActiveDragState | null>(null);
  const baseItemsRef = useRef(items);
  const latestItemsRef = useRef(items);
  const latestReorderItemsRef = useRef(reorderItems);
  const latestOnReorderRef = useRef(onReorder);

  useEffect(() => {
    latestItemsRef.current = items;

    if (!activeDragRef.current) {
      baseItemsRef.current = items;
      setDisplayItems(items);
    }
  }, [items]);

  useEffect(() => {
    latestReorderItemsRef.current = reorderItems;
  }, [reorderItems]);

  useEffect(() => {
    latestOnReorderRef.current = onReorder;
  }, [onReorder]);

  const clearDragState = useCallback((resetPreview = false) => {
    activeDragRef.current = null;
    setDraggingId(null);
    setDragOverId(null);

    if (resetPreview) {
      baseItemsRef.current = latestItemsRef.current;
      setDisplayItems(latestItemsRef.current);
    }
  }, []);

  useEffect(() => {
    if (disabled || items.length <= 1) {
      clearDragState(true);
    }
  }, [clearDragState, disabled, items.length]);

  useEffect(() => {
    if (!activeDragRef.current) {
      return;
    }

    function resolveTargetId(event: PointerEvent) {
      const element = document.elementFromPoint(event.clientX, event.clientY);
      if (!(element instanceof HTMLElement)) {
        return null;
      }

      const reorderElement = element.closest<HTMLElement>("[data-reorder-item-id]");
      return reorderElement?.dataset.reorderItemId?.trim() || null;
    }

    function updatePreview(targetId: string) {
      const currentDrag = activeDragRef.current;
      if (!currentDrag) {
        return;
      }

      const nextItems = latestReorderItemsRef.current(
        baseItemsRef.current,
        currentDrag.draggingId,
        targetId,
      );

      setDisplayItems(nextItems ?? baseItemsRef.current);
    }

    function handlePointerMove(event: PointerEvent) {
      const currentDrag = activeDragRef.current;
      if (!currentDrag || event.pointerId !== currentDrag.pointerId) {
        return;
      }

      const targetId = resolveTargetId(event);
      if (!targetId) {
        return;
      }

      setDragOverId(targetId);
      updatePreview(targetId);
    }

    async function finishDrag(event: PointerEvent) {
      const currentDrag = activeDragRef.current;
      if (!currentDrag || event.pointerId !== currentDrag.pointerId) {
        return;
      }

      const targetId = resolveTargetId(event) || dragOverId;
      const nextItems = targetId
        ? latestReorderItemsRef.current(
            baseItemsRef.current,
            currentDrag.draggingId,
            targetId,
          )
        : null;

      clearDragState();

      if (!targetId || !nextItems) {
        setDisplayItems(baseItemsRef.current);
        return;
      }

      setDisplayItems(nextItems);

      try {
        await latestOnReorderRef.current({
          draggingId: currentDrag.draggingId,
          targetId,
          nextItems,
        });
      } catch {
        baseItemsRef.current = latestItemsRef.current;
        setDisplayItems(latestItemsRef.current);
      }
    }

    function cancelDrag(event: PointerEvent) {
      const currentDrag = activeDragRef.current;
      if (!currentDrag || event.pointerId !== currentDrag.pointerId) {
        return;
      }

      clearDragState(true);
    }

    window.addEventListener("pointermove", handlePointerMove, true);
    window.addEventListener("pointerup", finishDrag, true);
    window.addEventListener("pointercancel", cancelDrag, true);

    return () => {
      window.removeEventListener("pointermove", handlePointerMove, true);
      window.removeEventListener("pointerup", finishDrag, true);
      window.removeEventListener("pointercancel", cancelDrag, true);
    };
  }, [clearDragState, dragOverId]);

  const handlePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLElement>, itemId: string) => {
      if (disabled || items.length <= 1 || event.button !== 0) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      baseItemsRef.current = latestItemsRef.current;
      activeDragRef.current = {
        draggingId: itemId,
        pointerId: event.pointerId,
      };
      setDisplayItems(latestItemsRef.current);
      setDraggingId(itemId);
      setDragOverId(itemId);
    },
    [disabled, items.length],
  );

  return {
    items: displayItems,
    draggingId,
    dragOverId,
    handlePointerDown,
    clearDragState: () => clearDragState(true),
  };
}
