import type { ReactNode } from "react";

import type { ScreenshotToolKind } from "@/features/screenshot-overlay/overlay-hotkeys";

type BusyAction = "copy" | "save" | "cancel" | null;
type ToolKind = ScreenshotToolKind;
type ShapeKind = "line" | "rect" | "ellipse" | "arrow";
type EffectKind = "mosaic" | "blur";
type TextStyleKind = "plain" | "outline" | "background" | "highlight";

type Point = { x: number; y: number };
type SelectionRect = { x: number; y: number; width: number; height: number };
type ToolbarIconAction = {
  key: string;
  label: string;
  icon: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  danger?: boolean;
  loading?: boolean;
};

type ShapeAnnotation = {
  id: string;
  kind: ShapeKind;
  color: string;
  strokeWidth: number;
  start: Point;
  end: Point;
};

type PenAnnotation = {
  id: string;
  kind: "pen";
  color: string;
  strokeWidth: number;
  points: Point[];
};

type TextAnnotation = {
  id: string;
  kind: "text";
  style: TextStyleKind;
  color: string;
  fontSize: number;
  rotation: number;
  opacity: number;
  point: Point;
  text: string;
};

type NumberAnnotation = {
  id: string;
  kind: "number";
  value: number;
  color: string;
  size: number;
  point: Point;
};

type FillAnnotation = {
  id: string;
  kind: "fill";
  color: string;
  opacity: number;
};

type EffectAnnotation = {
  id: string;
  kind: "effect";
  effect: EffectKind;
  intensity: number;
  start: Point;
  end: Point;
};

type Annotation =
  | ShapeAnnotation
  | PenAnnotation
  | TextAnnotation
  | NumberAnnotation
  | FillAnnotation
  | EffectAnnotation;
type Draft = ShapeAnnotation | PenAnnotation | EffectAnnotation | null;

type TextEditorState = {
  id: string;
  sourceAnnotationId: string | null;
  point: Point;
  text: string;
  style: TextStyleKind;
  color: string;
  fontSize: number;
  rotation: number;
  opacity: number;
};

type TextDragState = {
  ids: string[];
  originPoints: Record<string, Point>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  guides: SnapGuide[];
  moved: boolean;
};

type EffectTransformMode = "move" | "n" | "s" | "e" | "w" | "nw" | "ne" | "sw" | "se";
type ShapeTransformMode = "move" | "start" | "end" | Exclude<EffectTransformMode, "move">;

type EffectTransformState = {
  id: string;
  mode: EffectTransformMode;
  startPointer: Point;
  originBounds: SelectionRect;
  previewBounds: SelectionRect;
  moved: boolean;
};

type ShapeTransformState = {
  id: string;
  mode: ShapeTransformMode;
  startPointer: Point;
  originAnnotation: ShapeAnnotation;
  previewAnnotation: ShapeAnnotation;
  moved: boolean;
};

type ShapeGroupDragState = {
  ids: string[];
  originAnnotations: Record<string, ShapeAnnotation>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  moved: boolean;
};

type PenTransformState = {
  id: string;
  startPointer: Point;
  originAnnotation: PenAnnotation;
  previewAnnotation: PenAnnotation;
  moved: boolean;
};

type NumberDragState = {
  id: string;
  startPointer: Point;
  originAnnotation: NumberAnnotation;
  previewAnnotation: NumberAnnotation;
  moved: boolean;
};

type PenGroupDragState = {
  ids: string[];
  originAnnotations: Record<string, PenAnnotation>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  moved: boolean;
};

type NumberGroupDragState = {
  ids: string[];
  originPoints: Record<string, Point>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  moved: boolean;
};

type EffectGroupDragState = {
  ids: string[];
  originBounds: Record<string, SelectionRect>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  moved: boolean;
};

type MixedGroupDragState = {
  ids: string[];
  originAnnotations: Record<string, ObjectSelectionAnnotation>;
  startPointer: Point;
  delta: Point;
  groupBounds: SelectionRect;
  moved: boolean;
};

type ObjectSelectionMarqueeState = {
  startPointer: Point;
  currentPointer: Point;
  additive: boolean;
};

type ObjectSelectionFamily = "text" | "shape" | "pen" | "number" | "effect";
type ObjectSelectionAnnotation =
  | TextAnnotation
  | ShapeAnnotation
  | PenAnnotation
  | NumberAnnotation
  | EffectAnnotation;

type ObjectMarqueeResolution = {
  family: ObjectSelectionFamily | null;
  ids: string[];
  primaryId: string | null;
  counts: Record<ObjectSelectionFamily, number>;
};

type ObjectSelectionBuckets = {
  text: string[];
  shape: string[];
  pen: string[];
  number: string[];
  effect: string[];
};

type SelectionStatusBarTone = "idle" | "preview" | "selection";
type SelectionStatusBarModel = {
  tone: SelectionStatusBarTone;
  title: string;
  subtitle: string;
  chips: string[];
};

type SnapGuide = {
  orientation: "vertical" | "horizontal";
  position: number;
  start: number;
  end: number;
  source: "selection" | "annotation";
};

type TextClipboardState = {
  items: TextAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type PenClipboardState = {
  items: PenAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type ShapeClipboardState = {
  items: ShapeAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type NumberClipboardState = {
  items: NumberAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type EffectClipboardState = {
  items: EffectAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type MixedClipboardState = {
  items: ObjectSelectionAnnotation[];
  groupBounds: SelectionRect;
  pasteCount: number;
};
type ObjectClipboardKind = "text" | "shape" | "pen" | "number" | "effect" | "mixed";

type TextMetrics = {
  width: number;
  height: number;
  lineHeight: number;
};

type EffectHandleDescriptor = {
  mode: Exclude<EffectTransformMode, "move">;
  point: Point;
  cursor: string;
};
type ShapeHandleDescriptor = {
  mode: Exclude<ShapeTransformMode, "move">;
  point: Point;
  cursor: string;
};

export type {
  Annotation,
  BusyAction,
  Draft,
  EffectAnnotation,
  EffectClipboardState,
  EffectGroupDragState,
  EffectHandleDescriptor,
  EffectKind,
  EffectTransformMode,
  EffectTransformState,
  FillAnnotation,
  MixedClipboardState,
  MixedGroupDragState,
  NumberAnnotation,
  NumberClipboardState,
  NumberDragState,
  NumberGroupDragState,
  ObjectClipboardKind,
  ObjectMarqueeResolution,
  ObjectSelectionAnnotation,
  ObjectSelectionBuckets,
  ObjectSelectionFamily,
  ObjectSelectionMarqueeState,
  PenAnnotation,
  PenClipboardState,
  PenGroupDragState,
  PenTransformState,
  Point,
  SelectionRect,
  SelectionStatusBarModel,
  SelectionStatusBarTone,
  ShapeAnnotation,
  ShapeClipboardState,
  ShapeGroupDragState,
  ShapeHandleDescriptor,
  ShapeKind,
  ShapeTransformMode,
  ShapeTransformState,
  SnapGuide,
  TextAnnotation,
  TextClipboardState,
  TextDragState,
  TextEditorState,
  TextMetrics,
  TextStyleKind,
  ToolbarIconAction,
  ToolKind,
};
