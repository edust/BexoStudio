import type {
  NativeInteractionEditableShape,
  NativeInteractionExclusionRect,
  NativeInteractionMode,
  NativeInteractionSelectionRect,
  NativeInteractionStateView,
} from "@/types/backend";

export type NativeInteractionRuntimeRequest = {
  sessionId: string;
  visible: boolean;
  exclusionRects: NativeInteractionExclusionRect[];
  mode: NativeInteractionMode;
  selection: NativeInteractionSelectionRect | null;
  activeShape: NativeInteractionEditableShape | null;
  shapeCandidates: NativeInteractionEditableShape[];
  color: string;
  strokeWidth: number;
};

export function areNativeInteractionStatesEqual(
  left: NativeInteractionStateView | null,
  right: NativeInteractionStateView | null,
): boolean {
  if (!left && !right) return true;
  if (!left || !right) return false;
  return (
    left.backendKind === right.backendKind &&
    left.lifecycleState === right.lifecycleState &&
    left.hasActiveSession === right.hasActiveSession &&
    areNativeEditableShapesEqual(left.activeShape ?? null, right.activeShape ?? null) &&
    areNativeEditableShapesEqual(left.activeShapeDraft ?? null, right.activeShapeDraft ?? null) &&
    left.hoveredHitRegion === right.hoveredHitRegion &&
    left.dragMode === right.dragMode &&
    left.selectionRevision === right.selectionRevision &&
    left.activeShapeRevision === right.activeShapeRevision &&
    left.interactionMode === right.interactionMode &&
    areSelectionRectsEqual(left.selection ?? null, right.selection ?? null) &&
    areSelectionRectsEqual(left.rectDraft ?? null, right.rectDraft ?? null)
  );
}

export function buildNativeInteractionRuntimeRequestKey(
  request: NativeInteractionRuntimeRequest,
): string {
  return JSON.stringify({
    sessionId: request.sessionId,
    visible: request.visible,
    mode: request.mode,
    exclusionRects: request.exclusionRects.map(roundRect),
    selection: request.selection ? roundRect(request.selection) : null,
    activeShape: request.activeShape ? roundShape(request.activeShape) : null,
    shapeCandidates: request.shapeCandidates.map(roundShape),
    color: request.color,
    strokeWidth: roundRuntimeGeometry(request.strokeWidth),
  });
}

function roundRect(rect: NativeInteractionSelectionRect): NativeInteractionSelectionRect {
  return {
    x: roundRuntimeGeometry(rect.x),
    y: roundRuntimeGeometry(rect.y),
    width: roundRuntimeGeometry(rect.width),
    height: roundRuntimeGeometry(rect.height),
  };
}

function roundShape(shape: NativeInteractionEditableShape) {
  return {
    id: shape.id,
    kind: shape.kind,
    color: shape.color,
    strokeWidth: roundRuntimeGeometry(shape.strokeWidth),
    start: {
      x: roundRuntimeGeometry(shape.start.x),
      y: roundRuntimeGeometry(shape.start.y),
    },
    end: {
      x: roundRuntimeGeometry(shape.end.x),
      y: roundRuntimeGeometry(shape.end.y),
    },
  };
}

function roundRuntimeGeometry(value: number): number {
  return Number(value.toFixed(3));
}

function areNativeEditableShapesEqual(
  left: NativeInteractionEditableShape | null,
  right: NativeInteractionEditableShape | null,
): boolean {
  if (!left && !right) return true;
  if (!left || !right) return false;
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.color === right.color &&
    nearlyEqual(left.strokeWidth, right.strokeWidth) &&
    arePointsEqual(left.start, right.start) &&
    arePointsEqual(left.end, right.end)
  );
}

function areSelectionRectsEqual(
  left: NativeInteractionSelectionRect | null,
  right: NativeInteractionSelectionRect | null,
): boolean {
  if (!left && !right) return true;
  if (!left || !right) return false;
  return (
    nearlyEqual(left.x, right.x) &&
    nearlyEqual(left.y, right.y) &&
    nearlyEqual(left.width, right.width) &&
    nearlyEqual(left.height, right.height)
  );
}

function arePointsEqual(
  left: NativeInteractionEditableShape["start"],
  right: NativeInteractionEditableShape["start"],
): boolean {
  return nearlyEqual(left.x, right.x) && nearlyEqual(left.y, right.y);
}

function nearlyEqual(left: number, right: number): boolean {
  return Math.abs(left - right) < 0.001;
}
