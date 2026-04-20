import { App, Button, Tooltip, Typography } from "antd";
import { save } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Circle,
  ClipboardPaste,
  Copy,
  CopyPlus,
  Droplets,
  Grid2x2,
  Hash,
  Highlighter,
  Minus,
  MousePointer2,
  PaintBucket,
  Pencil,
  Plus,
  Redo2,
  RotateCcw,
  RotateCw,
  Save,
  Square,
  SquareDashed,
  Trash2,
  Type,
  Undo2,
  X,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import {
  cancelScreenshotSession,
  copyScreenshotSelection,
  getErrorSummary,
  getScreenshotSelectionRender,
  getScreenshotSession,
  hasDesktopRuntime,
  listenToNativeInteractionShapeAnnotationCommittedEvents,
  listenToNativeInteractionShapeAnnotationUpdatedEvents,
  listenToNativeInteractionStateUpdatedEvents,
  listenToScreenshotEscapePressedEvents,
  listenToScreenshotSessionUpdatedEvents,
  saveScreenshotSelection,
  updateNativeInteractionRuntime,
} from "@/lib/command-client";
import type {
  NativeInteractionEditableShape,
  NativeInteractionExclusionRect,
  NativeInteractionMode,
  NativeInteractionStateView,
  ScreenshotRenderedImageInput,
  ScreenshotSelectionInput,
  ScreenshotSelectionRenderView,
  ScreenshotSessionView,
} from "@/types/backend";

let tauriLogModulePromise: Promise<typeof import("@tauri-apps/plugin-log")> | null = null;

type BusyAction = "copy" | "save" | "cancel" | null;
type ToolKind = "select" | "line" | "rect" | "ellipse" | "arrow" | "pen" | "text" | "number" | "fill" | "mosaic" | "blur";
type ShapeKind = "line" | "rect" | "ellipse" | "arrow";
type EffectKind = "mosaic" | "blur";
type TextStyleKind = "plain" | "outline" | "background" | "highlight";

type Point = { x: number; y: number };
type SelectionRect = { x: number; y: number; width: number; height: number };
type NativeInteractionRuntimeRequest = {
  sessionId: string;
  visible: boolean;
  exclusionRects: NativeInteractionExclusionRect[];
  mode: NativeInteractionMode;
  selection: SelectionRect | null;
  activeShape: NativeInteractionEditableShape | null;
  shapeCandidates: NativeInteractionEditableShape[];
  color: string;
  strokeWidth: number;
};

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

type Annotation = ShapeAnnotation | PenAnnotation | TextAnnotation | NumberAnnotation | FillAnnotation | EffectAnnotation;
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

type ObjectSelectionAnnotation = TextAnnotation | ShapeAnnotation | PenAnnotation | NumberAnnotation | EffectAnnotation;

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

const TOOLS: Array<{ key: ToolKind; label: string }> = [
  { key: "select", label: "选区" },
  { key: "line", label: "线" },
  { key: "rect", label: "框" },
  { key: "ellipse", label: "圆" },
  { key: "arrow", label: "箭头" },
  { key: "pen", label: "画笔" },
  { key: "text", label: "文字" },
  { key: "number", label: "编号" },
  { key: "fill", label: "填色" },
  { key: "mosaic", label: "马赛克" },
  { key: "blur", label: "模糊" },
];

const DEFAULT_ANNOTATION_COLOR = "#00d08f";
const TEXT_STYLE_OPTIONS: Array<{ key: TextStyleKind; label: string }> = [
  { key: "plain", label: "纯色" },
  { key: "outline", label: "描边" },
  { key: "background", label: "背景" },
  { key: "highlight", label: "高亮" },
];
const NATIVE_SELECTION_RUNTIME_STABILIZE_MS = 220;
const FIXED_SCREENSHOT_TOOL_HOTKEYS: Array<[string, ToolKind]> = [
  ["1", "select"],
  ["2", "line"],
  ["3", "rect"],
  ["4", "ellipse"],
  ["5", "arrow"],
  ["6", "pen"],
  ["7", "text"],
  ["8", "fill"],
  ["9", "mosaic"],
  ["0", "blur"],
  ["n", "number"],
];
const TOOL_HOTKEY_MAP = buildToolHotkeyMap();

export default function ScreenshotOverlayPage() {
  const { message } = App.useApp();
  const runtimeAvailable = hasDesktopRuntime();
  const stageRef = useRef<HTMLDivElement | null>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const toolbarRef = useRef<HTMLDivElement | null>(null);
  const textEditorRef = useRef<HTMLTextAreaElement | null>(null);
  const annotationsRef = useRef<Annotation[]>([]);
  const textEditorStateRef = useRef<TextEditorState | null>(null);
  const textCompositionRef = useRef(false);
  const textClipboardRef = useRef<TextClipboardState | null>(null);
  const shapeClipboardRef = useRef<ShapeClipboardState | null>(null);
  const penClipboardRef = useRef<PenClipboardState | null>(null);
  const numberClipboardRef = useRef<NumberClipboardState | null>(null);
  const effectClipboardRef = useRef<EffectClipboardState | null>(null);
  const mixedClipboardRef = useRef<MixedClipboardState | null>(null);
  const objectClipboardKindRef = useRef<ObjectClipboardKind | null>(null);
  const previewRenderableRef = useRef<CanvasImageSource | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const pendingSessionIdRef = useRef<string | null>(null);
  const frozenToolbarRectRef = useRef<SelectionRect | null>(null);
  const selectionRedrawStartedWithSelectionRef = useRef(false);
  const pendingSelectionRedrawOriginRef = useRef<Point | null>(null);
  const nativeSelectionRuntimeBlockedUntilRef = useRef(0);
  const previousNativeSelectionDragModeRef = useRef<string | null>(null);
  const sessionLoadingSeenAtRef = useRef<Map<string, number>>(new Map());
  const firstPaintSessionIdRef = useRef<string | null>(null);
  const nativeInteractionRuntimeSeqRef = useRef(0);
  const nativeInteractionRuntimeInFlightRef = useRef(false);
  const nativeInteractionRuntimeInFlightKeyRef = useRef<string | null>(null);
  const nativeInteractionRuntimeAppliedKeyRef = useRef<string | null>(null);
  const nativeInteractionRuntimeQueuedRequestRef =
    useRef<NativeInteractionRuntimeRequest | null>(null);

  const [session, setSession] = useState<ScreenshotSessionView | null>(null);
  const [selection, setSelection] = useState<SelectionRect | null>(null);
  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [nativeSelectionRuntimeBlockedUntil, setNativeSelectionRuntimeBlockedUntil] =
    useState(0);
  const [toolbarMeasuredWidth, setToolbarMeasuredWidth] = useState(480);

  const [tool, setTool] = useState<ToolKind>("select");
  const color = DEFAULT_ANNOTATION_COLOR;
  const [strokeWidth, setStrokeWidth] = useState<number>(3);
  const [fontSize, setFontSize] = useState<number>(22);
  const [textStyle, setTextStyle] = useState<TextStyleKind>("plain");
  const [textRotation, setTextRotation] = useState<number>(0);
  const [textOpacity, setTextOpacity] = useState<number>(100);
  const [fillOpacity, setFillOpacity] = useState<number>(24);
  const [mosaicSize, setMosaicSize] = useState<number>(14);
  const [blurRadius, setBlurRadius] = useState<number>(10);

  const [annotations, setAnnotations] = useState<Annotation[]>([]);
  const [historyStack, setHistoryStack] = useState<Annotation[][]>([]);
  const [redoStack, setRedoStack] = useState<Annotation[][]>([]);
  const [draft, setDraft] = useState<Draft>(null);
  const [textEditor, setTextEditor] = useState<TextEditorState | null>(null);
  const [activeTextId, setActiveTextId] = useState<string | null>(null);
  const [selectedTextIds, setSelectedTextIds] = useState<string[]>([]);
  const [selectedShapeIds, setSelectedShapeIds] = useState<string[]>([]);
  const [selectedShapeId, setSelectedShapeId] = useState<string | null>(null);
  const [selectedPenIds, setSelectedPenIds] = useState<string[]>([]);
  const [selectedPenId, setSelectedPenId] = useState<string | null>(null);
  const [selectedEffectIds, setSelectedEffectIds] = useState<string[]>([]);
  const [selectedEffectId, setSelectedEffectId] = useState<string | null>(null);
  const [selectedNumberIds, setSelectedNumberIds] = useState<string[]>([]);
  const [selectedNumberId, setSelectedNumberId] = useState<string | null>(null);
  const [shapeTransform, setShapeTransform] = useState<ShapeTransformState | null>(null);
  const [shapeGroupDrag, setShapeGroupDrag] = useState<ShapeGroupDragState | null>(null);
  const [penTransform, setPenTransform] = useState<PenTransformState | null>(null);
  const [effectTransform, setEffectTransform] = useState<EffectTransformState | null>(null);
  const [penGroupDrag, setPenGroupDrag] = useState<PenGroupDragState | null>(null);
  const [numberDrag, setNumberDrag] = useState<NumberDragState | null>(null);
  const [numberGroupDrag, setNumberGroupDrag] = useState<NumberGroupDragState | null>(null);
  const [effectGroupDrag, setEffectGroupDrag] = useState<EffectGroupDragState | null>(null);
  const [mixedGroupDrag, setMixedGroupDrag] = useState<MixedGroupDragState | null>(null);
  const [objectSelectionMarquee, setObjectSelectionMarquee] = useState<ObjectSelectionMarqueeState | null>(null);
  const [textDrag, setTextDrag] = useState<TextDragState | null>(null);
  const [previewSurfaceReady, setPreviewSurfaceReady] = useState(false);
  const [previewImageVersion, setPreviewImageVersion] = useState<number>(0);
  const [nativeInteractionState, setNativeInteractionState] = useState<NativeInteractionStateView | null>(null);
  const previousToolRef = useRef<ToolKind>("select");
  const toolHotkeyMap = TOOL_HOTKEY_MAP;

  const activeRect = useMemo<SelectionRect | null>(() => {
    if (dragStart && dragCurrent && tool === "select" && !textDrag && !shapeGroupDrag && !numberGroupDrag && !effectGroupDrag && !mixedGroupDrag && !objectSelectionMarquee) {
      return normalizeRect(dragStart, dragCurrent);
    }
    return selection;
  }, [dragCurrent, dragStart, effectGroupDrag, mixedGroupDrag, numberGroupDrag, objectSelectionMarquee, selection, shapeGroupDrag, textDrag, tool]);

  const objectSelectionRect = useMemo<SelectionRect | null>(() => {
    if (!objectSelectionMarquee) {
      return null;
    }
    return normalizeRect(objectSelectionMarquee.startPointer, objectSelectionMarquee.currentPointer);
  }, [objectSelectionMarquee]);

  const previewImageSource = useMemo(() => {
    if (!session || session.imageStatus !== "ready") {
      return "";
    }
    if (session.previewTransport === "raw_rgba_fast") {
      return buildPreviewProtocolUrl(`session:${session.sessionId}`);
    }

    const previewImagePath = session.previewImagePath?.trim();
    if (previewImagePath) {
      return buildPreviewProtocolUrl(previewImagePath);
    }

    return session.imageDataUrl ?? "";
  }, [session]);

  const previewImageSourceKind = useMemo(() => {
    if (!session || session.imageStatus !== "ready") {
      return "";
    }
    if (session.previewTransport === "raw_rgba_fast") {
      return "raw_protocol";
    }
    if (session.previewImagePath?.trim()) {
      return "file";
    }
    return "data_url";
  }, [session]);

  const renderWebviewPreviewSurface = Boolean(session && !session.nativePreviewActive);

  const sessionImageReady = Boolean(
    session &&
      session.imageStatus === "ready" &&
      (session.nativePreviewActive || previewSurfaceReady),
  );

  const selectedRuntimeShapeIds = useMemo(
    () => (selectedShapeIds.length > 0 ? selectedShapeIds : selectedShapeId ? [selectedShapeId] : []),
    [selectedShapeId, selectedShapeIds],
  );

  const selectedRuntimeShapeAnnotations = useMemo(
    () =>
      selectedRuntimeShapeIds
        .map((id) => findShapeAnnotationById(annotations, id))
        .filter((annotation): annotation is ShapeAnnotation => annotation !== null),
    [annotations, selectedRuntimeShapeIds],
  );

  const runtimeSelectedFamilyCount = useMemo(
    () =>
      [
        (selectedTextIds.length > 0 ? selectedTextIds.length : activeTextId ? 1 : 0) > 0,
        selectedRuntimeShapeAnnotations.length > 0,
        (selectedPenIds.length > 0 ? selectedPenIds.length : selectedPenId ? 1 : 0) > 0,
        (selectedNumberIds.length > 0 ? selectedNumberIds.length : selectedNumberId ? 1 : 0) > 0,
        (selectedEffectIds.length > 0 ? selectedEffectIds.length : selectedEffectId ? 1 : 0) > 0,
      ].filter(Boolean).length,
    [
      activeTextId,
      selectedEffectId,
      selectedEffectIds.length,
      selectedNumberId,
      selectedNumberIds.length,
      selectedPenId,
      selectedPenIds.length,
      selectedRuntimeShapeAnnotations.length,
      selectedTextIds.length,
    ],
  );

  const nativeBaseSelectionEligible = Boolean(
    runtimeAvailable &&
      session &&
      session.nativePreviewActive &&
      tool === "select" &&
      annotations.length === 0 &&
      !textEditor &&
      !draft &&
      !busyAction,
  );

  const nativeShapeAnnotationEligible = Boolean(
    runtimeAvailable &&
      session &&
      session.nativePreviewActive &&
      selection &&
      (tool === "line" || tool === "rect" || tool === "ellipse" || tool === "arrow") &&
      !textEditor &&
      !busyAction &&
      !draft &&
      !dragStart &&
      !dragCurrent &&
      !shapeTransform &&
      !shapeGroupDrag &&
      !penTransform &&
      !penGroupDrag &&
      !effectTransform &&
      !effectGroupDrag &&
      !numberDrag &&
      !numberGroupDrag &&
      !mixedGroupDrag &&
      !objectSelectionMarquee &&
      !textDrag,
  );

  const nativeSingleShapeEditEligible = Boolean(
    runtimeAvailable &&
      session &&
      session.nativePreviewActive &&
      selection &&
      tool === "select" &&
      selectedRuntimeShapeAnnotations.length === 1 &&
      runtimeSelectedFamilyCount === 1 &&
      !textEditor &&
      !busyAction &&
      !draft &&
      !dragStart &&
      !dragCurrent &&
      !shapeTransform &&
      !shapeGroupDrag &&
      !penTransform &&
      !penGroupDrag &&
      !effectTransform &&
      !effectGroupDrag &&
      !numberDrag &&
      !numberGroupDrag &&
      !mixedGroupDrag &&
      !objectSelectionMarquee &&
      !textDrag,
  );

  const nativeInteractionActiveShape = useMemo<NativeInteractionEditableShape | null>(() => {
    if (!nativeSingleShapeEditEligible) {
      return null;
    }
    const annotation = selectedRuntimeShapeAnnotations[0];
    if (!annotation) {
      return null;
    }
    return {
      id: annotation.id,
      kind: annotation.kind,
      color: annotation.color,
      strokeWidth: annotation.strokeWidth,
      start: annotation.start,
      end: annotation.end,
    };
  }, [nativeSingleShapeEditEligible, selectedRuntimeShapeAnnotations]);

  const nativeInteractionRuntimeVisible =
    nativeBaseSelectionEligible || nativeShapeAnnotationEligible || nativeSingleShapeEditEligible;
  const nativeInteractionRuntimeMode: NativeInteractionMode =
    tool === "line" && nativeShapeAnnotationEligible
      ? "line_annotation"
      : tool === "ellipse" && nativeShapeAnnotationEligible
      ? "ellipse_annotation"
      : tool === "arrow" && nativeShapeAnnotationEligible
        ? "arrow_annotation"
        : tool === "rect" && nativeShapeAnnotationEligible
          ? "rect_annotation"
        : "selection";
  const nativeInteractionRuntimeSelection =
    nativeInteractionRuntimeMode === "selection" ? null : selection;

  const nativeSelectionDragActive =
    nativeInteractionRuntimeMode === "selection" &&
    (nativeInteractionState?.dragMode === "creating" ||
      nativeInteractionState?.dragMode === "moving" ||
      nativeInteractionState?.dragMode === "resizing");
  const nativeSelectionRuntimeCoolingDown =
    nativeInteractionRuntimeMode === "selection" &&
    nativeSelectionRuntimeBlockedUntil > performance.now();
  const nativeSelectionInteractionStabilizing =
    nativeSelectionDragActive || nativeSelectionRuntimeCoolingDown;

  const nativeShapeDragActive =
    nativeInteractionState?.dragMode === "shape_moving" ||
    nativeInteractionState?.dragMode === "shape_start_moving" ||
    nativeInteractionState?.dragMode === "shape_end_moving" ||
    nativeInteractionState?.dragMode === "shape_resizing";

  const nativeInteractionShapeCandidates = useMemo<NativeInteractionEditableShape[]>(() => {
    if (
      !selection ||
      !session?.nativePreviewActive ||
      (tool === "select" && nativeSelectionDragActive)
    ) {
      return [];
    }

    return annotations
      .filter(
        (annotation): annotation is ShapeAnnotation =>
          annotation.kind === "line" ||
          annotation.kind === "rect" ||
          annotation.kind === "ellipse" ||
          annotation.kind === "arrow",
      )
      .filter((annotation) =>
        doRectsIntersect(
          expandRect(
            resolveShapeAnnotationBounds(annotation),
            Math.max(6, annotation.strokeWidth / 2 + 4),
          ),
          selection,
        ),
      )
      .map((annotation) => ({
        id: annotation.id,
        kind: annotation.kind,
        color: annotation.color,
        strokeWidth: annotation.strokeWidth,
        start: annotation.start,
        end: annotation.end,
      }));
  }, [annotations, nativeSelectionDragActive, selection, session?.nativePreviewActive, tool]);

  const nativeBaseSelectionActive = Boolean(
    nativeBaseSelectionEligible &&
      nativeInteractionState?.hasActiveSession &&
      (nativeInteractionState.lifecycleState === "visible" ||
        nativeInteractionState.lifecycleState === "prepared"),
  );

  const nativeShapeAnnotationActive = Boolean(
    nativeShapeAnnotationEligible &&
      nativeInteractionState?.hasActiveSession &&
      (nativeInteractionState.interactionMode === "line_annotation" ||
        nativeInteractionState.interactionMode === "rect_annotation" ||
        nativeInteractionState.interactionMode === "ellipse_annotation" ||
        nativeInteractionState.interactionMode === "arrow_annotation") &&
      (nativeInteractionState.lifecycleState === "visible" ||
        nativeInteractionState.lifecycleState === "prepared"),
  );

  const nativeSingleShapeEditActive = Boolean(
    nativeSingleShapeEditEligible &&
      nativeInteractionState?.hasActiveSession &&
      nativeInteractionState.activeShape?.id &&
      (nativeInteractionState.lifecycleState === "visible" ||
        nativeInteractionState.lifecycleState === "prepared"),
  );

  const nativeInteractionVisualsActive =
    nativeBaseSelectionActive || nativeShapeAnnotationActive || nativeSingleShapeEditActive;
  const nativeInteractionPointerOwned =
    nativeBaseSelectionEligible || nativeShapeAnnotationEligible || nativeSingleShapeEditEligible;

  const nativeManagedShapeIds = useMemo(() => {
    const next = new Set<string>();
    if (nativeSingleShapeEditActive) {
      const activeShapeId =
        nativeInteractionState?.activeShapeDraft?.id ??
        nativeInteractionState?.activeShape?.id ??
        selectedRuntimeShapeAnnotations[0]?.id ??
        null;
      if (activeShapeId) {
        next.add(activeShapeId);
      }
    }
    return next;
  }, [nativeInteractionState?.activeShape?.id, nativeInteractionState?.activeShapeDraft?.id, nativeSingleShapeEditActive, selectedRuntimeShapeAnnotations]);

  const displayAnnotations = useMemo(
    () =>
      buildDisplayAnnotations(
        annotations,
        textEditor,
        textDrag,
        shapeTransform,
        shapeGroupDrag,
        penTransform,
        penGroupDrag,
        effectTransform,
        numberDrag,
        numberGroupDrag,
        effectGroupDrag,
        mixedGroupDrag,
        Array.from(nativeManagedShapeIds),
      ),
    [
      annotations,
      effectGroupDrag,
      effectTransform,
      mixedGroupDrag,
      nativeManagedShapeIds,
      numberDrag,
      numberGroupDrag,
      penGroupDrag,
      penTransform,
      shapeGroupDrag,
      shapeTransform,
      textDrag,
      textEditor,
    ],
  );

  const effectPreviewAnnotations = useMemo(() => {
    const next = displayAnnotations.filter((annotation): annotation is EffectAnnotation => annotation.kind === "effect");
    if (draft?.kind === "effect") {
      next.push(draft);
    }
    return next;
  }, [displayAnnotations, draft]);
  const hasEffectPreview = effectPreviewAnnotations.length > 0;

  const selectedTextAnnotations = useMemo(
    () =>
      selectedTextIds
        .map((id) => findTextAnnotationById(displayAnnotations, id))
        .filter((annotation): annotation is TextAnnotation => annotation !== null),
    [displayAnnotations, selectedTextIds],
  );

  const selectedTextAnnotation = useMemo(
    () => (activeTextId ? findTextAnnotationById(displayAnnotations, activeTextId) : null),
    [activeTextId, displayAnnotations],
  );

  const selectedEffectAnnotations = useMemo(
    () =>
      (selectedEffectIds.length > 0 ? selectedEffectIds : selectedEffectId ? [selectedEffectId] : [])
        .map((id) => findEffectAnnotationById(displayAnnotations, id))
        .filter((annotation): annotation is EffectAnnotation => annotation !== null),
    [displayAnnotations, selectedEffectId, selectedEffectIds],
  );

  const selectedEffectAnnotation = useMemo(
    () => (selectedEffectId ? findEffectAnnotationById(displayAnnotations, selectedEffectId) : null),
    [displayAnnotations, selectedEffectId],
  );

  const selectedShapeAnnotation = useMemo(
    () => (selectedShapeId ? findShapeAnnotationById(annotations, selectedShapeId) : null),
    [annotations, selectedShapeId],
  );

  const selectedShapeAnnotations = useMemo(
    () =>
      (selectedShapeIds.length > 0 ? selectedShapeIds : selectedShapeId ? [selectedShapeId] : [])
        .map((id) => findShapeAnnotationById(annotations, id))
        .filter((annotation): annotation is ShapeAnnotation => annotation !== null),
    [annotations, selectedShapeId, selectedShapeIds],
  );

  const selectedPenAnnotations = useMemo(
    () =>
      (selectedPenIds.length > 0 ? selectedPenIds : selectedPenId ? [selectedPenId] : [])
        .map((id) => findPenAnnotationById(displayAnnotations, id))
        .filter((annotation): annotation is PenAnnotation => annotation !== null),
    [displayAnnotations, selectedPenId, selectedPenIds],
  );

  const selectedPenAnnotation = useMemo(
    () => (selectedPenId ? findPenAnnotationById(displayAnnotations, selectedPenId) : null),
    [displayAnnotations, selectedPenId],
  );

  const selectedNumberAnnotations = useMemo(
    () =>
      (selectedNumberIds.length > 0 ? selectedNumberIds : selectedNumberId ? [selectedNumberId] : [])
        .map((id) => findNumberAnnotationById(displayAnnotations, id))
        .filter((annotation): annotation is NumberAnnotation => annotation !== null),
    [displayAnnotations, selectedNumberId, selectedNumberIds],
  );

  const selectedNumberAnnotation = useMemo(
    () => (selectedNumberId ? findNumberAnnotationById(displayAnnotations, selectedNumberId) : null),
    [displayAnnotations, selectedNumberId],
  );

  const selectedFamilyCount = useMemo(
    () =>
      [
        selectedTextAnnotations.length > 0,
        selectedShapeAnnotations.length > 0,
        selectedPenAnnotations.length > 0,
        selectedNumberAnnotations.length > 0,
        selectedEffectAnnotations.length > 0,
      ].filter(Boolean).length,
    [selectedEffectAnnotations.length, selectedNumberAnnotations.length, selectedPenAnnotations.length, selectedShapeAnnotations.length, selectedTextAnnotations.length],
  );

  const totalSelectedObjectCount = useMemo(
    () =>
      selectedTextAnnotations.length +
      selectedShapeAnnotations.length +
      selectedPenAnnotations.length +
      selectedNumberAnnotations.length +
      selectedEffectAnnotations.length,
    [selectedEffectAnnotations.length, selectedNumberAnnotations.length, selectedPenAnnotations.length, selectedShapeAnnotations.length, selectedTextAnnotations.length],
  );

  const hasMixedFamilySelection = selectedFamilyCount > 1;

  const activeSelectionGroupOverlay = useMemo<{ rect: SelectionRect; items: string[] } | null>(() => {
    if (selectedFamilyCount > 1) {
      return {
        rect: resolveObjectSelectionGroupBounds([
          ...selectedTextAnnotations,
          ...selectedShapeAnnotations,
          ...selectedPenAnnotations,
          ...selectedNumberAnnotations,
          ...selectedEffectAnnotations,
        ]),
        items: [
          `已混选 ${totalSelectedObjectCount} 个对象 / ${selectedFamilyCount} 个家族`,
          "整组拖动",
          "方向键/Shift 微调",
          "Ctrl/Cmd+C/V/D",
          "Delete 删除",
          "Ctrl+[ / ] 层级",
          "Ctrl/Cmd+A 全选",
        ],
      };
    }

    if (selectedTextAnnotations.length > 1) {
      return {
        rect: resolveTextGroupBounds(selectedTextAnnotations),
        items: [
          `已选 ${selectedTextAnnotations.length} 个文字，可整组拖动 / 复制`,
          "方向键/Shift 微调",
          "Ctrl+[ / ] 层级",
          "Ctrl+Shift+[ / ] 置底/置顶",
        ],
      };
    }

    if (selectedShapeAnnotations.length > 1) {
      return {
        rect: resolveShapeGroupBounds(selectedShapeAnnotations),
        items: [
          `已选 ${selectedShapeAnnotations.length} 个图形，可整组拖动 / 复制 / 粘贴 / 重复`,
          "批量改线宽",
          "方向键/Shift 微调",
          "Ctrl+[ / ] 层级",
          "Ctrl+Shift+[ / ] 置底/置顶",
        ],
      };
    }

    if (selectedPenAnnotations.length > 1) {
      return {
        rect: resolvePenGroupBounds(selectedPenAnnotations),
        items: [
          `已选 ${selectedPenAnnotations.length} 条画笔，可整组拖动 / 复制`,
          "方向键/Shift 微调",
          "Ctrl+[ / ] 层级",
          "Ctrl+Shift+[ / ] 置底/置顶",
        ],
      };
    }

    if (selectedNumberAnnotations.length > 1) {
      return {
        rect: resolveNumberGroupBounds(selectedNumberAnnotations),
        items: [
          `已选 ${selectedNumberAnnotations.length} 个编号，可整组拖动 / 复制`,
          "方向键/Shift 微调",
          "Ctrl+[ / ] 层级",
          "Ctrl+Shift+[ / ] 置底/置顶",
        ],
      };
    }

    if (selectedEffectAnnotations.length > 1) {
      return {
        rect: resolveEffectGroupBounds(selectedEffectAnnotations),
        items: [
          `已选 ${selectedEffectAnnotations.length} 个效果区域，可整组拖动 / 复制`,
          "方向键/Shift 微调",
          "Ctrl+[ / ] 层级",
          "Ctrl+Shift+[ / ] 置底/置顶",
        ],
      };
    }

    return null;
  }, [selectedEffectAnnotations, selectedFamilyCount, selectedNumberAnnotations, selectedPenAnnotations, selectedShapeAnnotations, selectedTextAnnotations, totalSelectedObjectCount]);

  const objectSelectionPreview = useMemo<ObjectMarqueeResolution | null>(() => {
    if (!objectSelectionRect) {
      return null;
    }

    return resolveObjectMarqueeSelection(
      displayAnnotations,
      objectSelectionRect,
      resolvePreferredObjectMarqueeFamily(
        selectedTextIds.length > 0 ? selectedTextIds : activeTextId ? [activeTextId] : [],
        selectedShapeIds.length > 0 ? selectedShapeIds : selectedShapeId ? [selectedShapeId] : [],
        selectedPenIds.length > 0 ? selectedPenIds : selectedPenId ? [selectedPenId] : [],
        selectedNumberIds.length > 0 ? selectedNumberIds : selectedNumberId ? [selectedNumberId] : [],
        selectedEffectIds.length > 0 ? selectedEffectIds : selectedEffectId ? [selectedEffectId] : [],
      ),
    );
  }, [activeTextId, displayAnnotations, objectSelectionRect, selectedEffectId, selectedEffectIds, selectedNumberId, selectedNumberIds, selectedPenId, selectedPenIds, selectedShapeId, selectedShapeIds, selectedTextIds]);

  const objectSelectionPreviewAnnotations = useMemo<ObjectSelectionAnnotation[]>(() => {
    if (!objectSelectionPreview?.family || objectSelectionPreview.ids.length === 0) {
      return [];
    }

    const selectedSet = new Set(objectSelectionPreview.ids);
    return displayAnnotations.filter((annotation): annotation is ObjectSelectionAnnotation => {
      if (!selectedSet.has(annotation.id)) {
        return false;
      }

      if (objectSelectionPreview.family === "text") {
        return annotation.kind === "text";
      }
      if (objectSelectionPreview.family === "shape") {
        return annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow";
      }
      if (objectSelectionPreview.family === "pen") {
        return annotation.kind === "pen";
      }
      if (objectSelectionPreview.family === "number") {
        return annotation.kind === "number";
      }
      return annotation.kind === "effect";
    });
  }, [displayAnnotations, objectSelectionPreview]);

  const setAnnotationSnapshot = useCallback((next: Annotation[]) => {
    const snapshot = cloneAnnotations(next);
    annotationsRef.current = snapshot;
    setAnnotations(snapshot);
  }, []);

  const syncTextControls = useCallback((annotation: TextAnnotation | null) => {
    if (!annotation) {
      return;
    }

    setTextStyle(annotation.style);
    setFontSize(annotation.fontSize);
    setTextRotation(Math.round(annotation.rotation));
    setTextOpacity(Math.round(annotation.opacity * 100));
  }, []);

  const syncEffectControls = useCallback((annotation: EffectAnnotation | null) => {
    if (!annotation) {
      return;
    }

    if (annotation.effect === "mosaic") {
      setMosaicSize(Math.round(annotation.intensity));
      return;
    }

    setBlurRadius(Math.round(annotation.intensity));
  }, []);

  const syncNumberControls = useCallback((annotation: NumberAnnotation | null) => {
    if (!annotation) {
      return;
    }

    setFontSize(annotation.size);
  }, []);

  const syncShapeControls = useCallback((annotation: ShapeAnnotation | null) => {
    if (!annotation) {
      return;
    }

    setStrokeWidth(annotation.strokeWidth);
  }, []);

  const syncPenControls = useCallback((annotation: PenAnnotation | null) => {
    if (!annotation) {
      return;
    }

    setStrokeWidth(annotation.strokeWidth);
  }, []);

  const clearTextSelection = useCallback(() => {
    setActiveTextId(null);
    setSelectedTextIds([]);
  }, []);

  const clearShapeSelection = useCallback(() => {
    setSelectedShapeIds([]);
    setSelectedShapeId(null);
  }, []);

  const clearPenSelection = useCallback(() => {
    setSelectedPenIds([]);
    setSelectedPenId(null);
  }, []);

  const clearEffectSelection = useCallback(() => {
    setSelectedEffectIds([]);
    setSelectedEffectId(null);
  }, []);

  const clearNumberSelection = useCallback(() => {
    setSelectedNumberIds([]);
    setSelectedNumberId(null);
  }, []);

  const setShapeSelection = useCallback(
    (ids: string[], primaryId?: string | null, sourceAnnotations: Annotation[] = annotationsRef.current, primaryAnnotation?: ShapeAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        clearShapeSelection();
        return;
      }

      const nextPrimaryId = primaryId && uniqueIds.includes(primaryId) ? primaryId : uniqueIds[uniqueIds.length - 1];
      const nextPrimaryAnnotation =
        primaryAnnotation && uniqueIds.includes(primaryAnnotation.id)
          ? primaryAnnotation
          : findShapeAnnotationById(sourceAnnotations, nextPrimaryId);

      setSelectedShapeIds(uniqueIds);
      setSelectedShapeId(nextPrimaryId);
      syncShapeControls(nextPrimaryAnnotation);
    },
    [clearShapeSelection, syncShapeControls],
  );

  const getSelectedShapeIds = useCallback(() => {
    if (selectedShapeIds.length > 0) {
      return selectedShapeIds;
    }
    return selectedShapeId ? [selectedShapeId] : [];
  }, [selectedShapeId, selectedShapeIds]);

  const selectShapeAnnotation = useCallback(
    (annotation: ShapeAnnotation | null, options?: { toggle?: boolean }) => {
      if (!annotation) {
        clearShapeSelection();
        return;
      }

      if (options?.toggle) {
        if (selectedShapeIds.includes(annotation.id)) {
          const remaining = selectedShapeIds.filter((id) => id !== annotation.id);
          if (remaining.length === 0) {
            clearShapeSelection();
            return;
          }
          const nextPrimaryId =
            selectedShapeId && remaining.includes(selectedShapeId) ? selectedShapeId : remaining[remaining.length - 1];
          setShapeSelection(remaining, nextPrimaryId);
          return;
        }

        setShapeSelection([...selectedShapeIds, annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }

      setShapeSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
    },
    [clearShapeSelection, selectedShapeId, selectedShapeIds, setShapeSelection],
  );

  const setPenSelection = useCallback(
    (ids: string[], primaryId?: string | null, sourceAnnotations: Annotation[] = annotationsRef.current, primaryAnnotation?: PenAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        clearPenSelection();
        return;
      }

      const nextPrimaryId = primaryId && uniqueIds.includes(primaryId) ? primaryId : uniqueIds[uniqueIds.length - 1];
      const nextPrimaryAnnotation =
        primaryAnnotation && uniqueIds.includes(primaryAnnotation.id)
          ? primaryAnnotation
          : findPenAnnotationById(sourceAnnotations, nextPrimaryId);

      setSelectedPenIds(uniqueIds);
      setSelectedPenId(nextPrimaryId);
      syncPenControls(nextPrimaryAnnotation);
    },
    [clearPenSelection, syncPenControls],
  );

  const getSelectedPenIds = useCallback(() => {
    if (selectedPenIds.length > 0) {
      return selectedPenIds;
    }
    return selectedPenId ? [selectedPenId] : [];
  }, [selectedPenId, selectedPenIds]);

  const selectPenAnnotation = useCallback(
    (annotation: PenAnnotation | null, options?: { toggle?: boolean }) => {
      if (!annotation) {
        clearPenSelection();
        return;
      }

      if (options?.toggle) {
        if (selectedPenIds.includes(annotation.id)) {
          const remaining = selectedPenIds.filter((id) => id !== annotation.id);
          if (remaining.length === 0) {
            clearPenSelection();
            return;
          }
          const nextPrimaryId =
            selectedPenId && remaining.includes(selectedPenId) ? selectedPenId : remaining[remaining.length - 1];
          setPenSelection(remaining, nextPrimaryId);
          return;
        }

        setPenSelection([...selectedPenIds, annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }

      setPenSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
    },
    [clearPenSelection, selectedPenId, selectedPenIds, setPenSelection],
  );

  const setTextSelection = useCallback(
    (ids: string[], primaryId?: string | null, sourceAnnotations: Annotation[] = annotationsRef.current, primaryAnnotation?: TextAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        clearTextSelection();
        return;
      }

      const nextPrimaryId = primaryId && uniqueIds.includes(primaryId) ? primaryId : uniqueIds[uniqueIds.length - 1];
      const nextPrimaryAnnotation =
        primaryAnnotation && uniqueIds.includes(primaryAnnotation.id)
          ? primaryAnnotation
          : findTextAnnotationById(sourceAnnotations, nextPrimaryId);

      setSelectedTextIds(uniqueIds);
      setActiveTextId(nextPrimaryId);
      syncTextControls(nextPrimaryAnnotation);
    },
    [clearTextSelection, syncTextControls],
  );

  const getSelectedTextIds = useCallback(() => {
    if (selectedTextIds.length > 0) {
      return selectedTextIds;
    }
    return activeTextId ? [activeTextId] : [];
  }, [activeTextId, selectedTextIds]);

  const setEffectSelection = useCallback(
    (ids: string[], primaryId?: string | null, sourceAnnotations: Annotation[] = annotationsRef.current, primaryAnnotation?: EffectAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        clearEffectSelection();
        return;
      }

      const nextPrimaryId = primaryId && uniqueIds.includes(primaryId) ? primaryId : uniqueIds[uniqueIds.length - 1];
      const nextPrimaryAnnotation =
        primaryAnnotation && uniqueIds.includes(primaryAnnotation.id)
          ? primaryAnnotation
          : findEffectAnnotationById(sourceAnnotations, nextPrimaryId);

      setSelectedEffectIds(uniqueIds);
      setSelectedEffectId(nextPrimaryId);
      syncEffectControls(nextPrimaryAnnotation);
    },
    [clearEffectSelection, syncEffectControls],
  );

  const getSelectedEffectIds = useCallback(() => {
    if (selectedEffectIds.length > 0) {
      return selectedEffectIds;
    }
    return selectedEffectId ? [selectedEffectId] : [];
  }, [selectedEffectId, selectedEffectIds]);

  const setNumberSelection = useCallback(
    (ids: string[], primaryId?: string | null, sourceAnnotations: Annotation[] = annotationsRef.current, primaryAnnotation?: NumberAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        clearNumberSelection();
        return;
      }

      const nextPrimaryId = primaryId && uniqueIds.includes(primaryId) ? primaryId : uniqueIds[uniqueIds.length - 1];
      const nextPrimaryAnnotation =
        primaryAnnotation && uniqueIds.includes(primaryAnnotation.id)
          ? primaryAnnotation
          : findNumberAnnotationById(sourceAnnotations, nextPrimaryId);

      setSelectedNumberIds(uniqueIds);
      setSelectedNumberId(nextPrimaryId);
      syncNumberControls(nextPrimaryAnnotation);
    },
    [clearNumberSelection, syncNumberControls],
  );

  const getSelectedNumberIds = useCallback(() => {
    if (selectedNumberIds.length > 0) {
      return selectedNumberIds;
    }
    return selectedNumberId ? [selectedNumberId] : [];
  }, [selectedNumberId, selectedNumberIds]);

  const selectTextAnnotation = useCallback(
    (annotation: TextAnnotation | null, options?: { toggle?: boolean; preserveGroup?: boolean }) => {
      if (!annotation) {
        clearTextSelection();
        return;
      }

      if (options?.toggle) {
        if (selectedTextIds.includes(annotation.id)) {
          const remaining = selectedTextIds.filter((id) => id !== annotation.id);
          if (remaining.length === 0) {
            clearTextSelection();
            return;
          }
          const nextPrimaryId =
            activeTextId && remaining.includes(activeTextId) ? activeTextId : remaining[remaining.length - 1];
          setTextSelection(remaining, nextPrimaryId);
          return;
        }

        setTextSelection([...selectedTextIds, annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }

      if (options?.preserveGroup && selectedTextIds.includes(annotation.id) && selectedTextIds.length > 0) {
        setTextSelection(selectedTextIds, annotation.id, annotationsRef.current, annotation);
        return;
      }

      setTextSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
    },
    [activeTextId, clearTextSelection, selectedTextIds, setTextSelection],
  );

  const selectEffectAnnotation = useCallback(
    (annotation: EffectAnnotation | null, options?: { toggle?: boolean }) => {
      if (!annotation) {
        clearEffectSelection();
        return;
      }

      if (options?.toggle) {
        if (selectedEffectIds.includes(annotation.id)) {
          const remaining = selectedEffectIds.filter((id) => id !== annotation.id);
          if (remaining.length === 0) {
            clearEffectSelection();
            return;
          }
          const nextPrimaryId =
            selectedEffectId && remaining.includes(selectedEffectId) ? selectedEffectId : remaining[remaining.length - 1];
          setEffectSelection(remaining, nextPrimaryId);
          return;
        }

        setEffectSelection([...selectedEffectIds, annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }

      setEffectSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
    },
    [clearEffectSelection, selectedEffectId, selectedEffectIds, setEffectSelection],
  );

  const selectNumberAnnotation = useCallback(
    (annotation: NumberAnnotation | null, options?: { toggle?: boolean }) => {
      if (!annotation) {
        clearNumberSelection();
        return;
      }

      if (options?.toggle) {
        if (selectedNumberIds.includes(annotation.id)) {
          const remaining = selectedNumberIds.filter((id) => id !== annotation.id);
          if (remaining.length === 0) {
            clearNumberSelection();
            return;
          }
          const nextPrimaryId =
            selectedNumberId && remaining.includes(selectedNumberId) ? selectedNumberId : remaining[remaining.length - 1];
          setNumberSelection(remaining, nextPrimaryId);
          return;
        }

        setNumberSelection([...selectedNumberIds, annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }

      setNumberSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
    },
    [clearNumberSelection, selectedNumberId, selectedNumberIds, setNumberSelection],
  );

  const updateTextEditor = useCallback((updater: (current: TextEditorState) => TextEditorState) => {
    setTextEditor((current) => {
      if (!current) return current;
      const next = updater(current);
      textEditorStateRef.current = next;
      return next;
    });
  }, []);

  const resetAnnotations = useCallback(() => {
    annotationsRef.current = [];
    textEditorStateRef.current = null;
    textCompositionRef.current = false;
    setAnnotations([]);
    setHistoryStack([]);
    setRedoStack([]);
    setDraft(null);
    setTextEditor(null);
    setActiveTextId(null);
    setSelectedTextIds([]);
    setSelectedShapeIds([]);
    setSelectedShapeId(null);
    setSelectedPenIds([]);
    setSelectedPenId(null);
    setSelectedEffectIds([]);
    setSelectedEffectId(null);
    setSelectedNumberIds([]);
    setSelectedNumberId(null);
    setShapeTransform(null);
    setShapeGroupDrag(null);
    setPenTransform(null);
    setPenGroupDrag(null);
    setEffectTransform(null);
    setNumberDrag(null);
    setNumberGroupDrag(null);
    setEffectGroupDrag(null);
    setMixedGroupDrag(null);
    setObjectSelectionMarquee(null);
    setTextDrag(null);
  }, []);

  const resetSessionInteractionState = useCallback(() => {
    setSelection(null);
    setDragStart(null);
    setDragCurrent(null);
    setNativeInteractionState(null);
    nativeSelectionRuntimeBlockedUntilRef.current = 0;
    selectionRedrawStartedWithSelectionRef.current = false;
    pendingSelectionRedrawOriginRef.current = null;
    textClipboardRef.current = null;
    shapeClipboardRef.current = null;
    penClipboardRef.current = null;
    numberClipboardRef.current = null;
    effectClipboardRef.current = null;
    mixedClipboardRef.current = null;
    objectClipboardKindRef.current = null;
    resetAnnotations();
    setTool("select");
  }, [resetAnnotations]);

  const commitAnnotations = useCallback(
    (next: Annotation[]) => {
      setHistoryStack((stack) => [...stack, cloneAnnotations(annotationsRef.current)]);
      setRedoStack([]);
      setAnnotationSnapshot(next);
    },
    [setAnnotationSnapshot],
  );

  const pushAnnotation = useCallback(
    (annotation: Annotation) => {
      commitAnnotations([...annotationsRef.current, annotation]);
      if (annotation.kind === "text") {
        clearEffectSelection();
        clearPenSelection();
        selectTextAnnotation(annotation);
        return;
      }
      if (annotation.kind === "effect") {
        clearTextSelection();
        clearShapeSelection();
        clearPenSelection();
        clearNumberSelection();
        selectEffectAnnotation(annotation);
        return;
      }
      if (annotation.kind === "number") {
        clearTextSelection();
        clearShapeSelection();
        clearPenSelection();
        clearEffectSelection();
        selectNumberAnnotation(annotation);
        return;
      }
      if (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") {
        clearTextSelection();
        clearPenSelection();
        clearEffectSelection();
        clearNumberSelection();
        selectShapeAnnotation(annotation);
        return;
      }
      if (annotation.kind === "pen") {
        clearTextSelection();
        clearShapeSelection();
        clearEffectSelection();
        clearNumberSelection();
        setPenSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
        return;
      }
      clearTextSelection();
      clearShapeSelection();
      clearPenSelection();
      clearEffectSelection();
      clearNumberSelection();
    },
    [clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, commitAnnotations, selectEffectAnnotation, selectNumberAnnotation, selectShapeAnnotation, selectTextAnnotation, setPenSelection],
  );

  const openTextEditor = useCallback(
    (point: Point, source?: TextAnnotation | null) => {
      const nextEditor: TextEditorState = {
        id: source?.id ?? crypto.randomUUID(),
        sourceAnnotationId: source?.id ?? null,
        point: source?.point ?? point,
        text: source?.text ?? "",
        style: source?.style ?? textStyle,
        color: source?.color ?? color,
        fontSize: source?.fontSize ?? fontSize,
        rotation: source?.rotation ?? textRotation,
        opacity: source?.opacity ?? textOpacity / 100,
      };

      textCompositionRef.current = false;
      textEditorStateRef.current = nextEditor;
      setTextEditor(nextEditor);
      setTextDrag(null);
      setTool("text");

      if (source) {
        selectTextAnnotation(source);
        return;
      }

      clearTextSelection();
      setTextStyle(nextEditor.style);
      setFontSize(nextEditor.fontSize);
      setTextRotation(Math.round(nextEditor.rotation));
      setTextOpacity(Math.round(nextEditor.opacity * 100));
    },
    [clearTextSelection, color, fontSize, selectTextAnnotation, textOpacity, textRotation, textStyle],
  );

  const cancelTextEditor = useCallback(() => {
    const current = textEditorStateRef.current;
    textCompositionRef.current = false;
    textEditorStateRef.current = null;
    setTextEditor(null);

    if (current?.sourceAnnotationId) {
      selectTextAnnotation(findTextAnnotationById(annotationsRef.current, current.sourceAnnotationId));
      return;
    }

    clearTextSelection();
  }, [clearTextSelection, selectTextAnnotation]);

  const commitTextEditor = useCallback((): TextAnnotation | null => {
    const current = textEditorStateRef.current;
    if (!current) return null;

    textCompositionRef.current = false;
    textEditorStateRef.current = null;
    setTextEditor(null);

    const text = normalizeTextContent(current.text);
    if (!text) {
      if (current.sourceAnnotationId) {
        selectTextAnnotation(findTextAnnotationById(annotationsRef.current, current.sourceAnnotationId));
      } else {
        clearTextSelection();
      }
      return null;
    }

    const annotation: TextAnnotation = {
      id: current.sourceAnnotationId ?? current.id,
      kind: "text",
      style: current.style,
      color: current.color,
      fontSize: current.fontSize,
      rotation: current.rotation,
      opacity: current.opacity,
      point: current.point,
      text,
    };

    if (current.sourceAnnotationId) {
      commitAnnotations(
        annotationsRef.current.map((item) => (item.id === current.sourceAnnotationId ? annotation : item)),
      );
    } else {
      commitAnnotations([...annotationsRef.current, annotation]);
    }

    selectTextAnnotation(annotation);
    return annotation;
  }, [clearTextSelection, commitAnnotations, selectTextAnnotation]);

  const undo = useCallback(() => {
    textCompositionRef.current = false;
    textEditorStateRef.current = null;
    setTextEditor(null);
    setShapeTransform(null);
    setShapeGroupDrag(null);
    setPenTransform(null);
    setPenGroupDrag(null);
    setEffectTransform(null);
    setNumberDrag(null);
    setNumberGroupDrag(null);
    setEffectGroupDrag(null);
    setObjectSelectionMarquee(null);
    setTextDrag(null);
    clearTextSelection();
    clearShapeSelection();
    clearPenSelection();
    clearEffectSelection();
    clearNumberSelection();

    setHistoryStack((stack) => {
      if (stack.length === 0) return stack;
      const previous = stack[stack.length - 1];
      setRedoStack((redo) => [cloneAnnotations(annotationsRef.current), ...redo]);
      setAnnotationSnapshot(previous);
      return stack.slice(0, stack.length - 1);
    });
  }, [clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, setAnnotationSnapshot]);

  const redo = useCallback(() => {
    textCompositionRef.current = false;
    textEditorStateRef.current = null;
    setTextEditor(null);
    setShapeTransform(null);
    setShapeGroupDrag(null);
    setPenTransform(null);
    setPenGroupDrag(null);
    setEffectTransform(null);
    setNumberDrag(null);
    setNumberGroupDrag(null);
    setEffectGroupDrag(null);
    setObjectSelectionMarquee(null);
    setTextDrag(null);
    clearTextSelection();
    clearShapeSelection();
    clearPenSelection();
    clearEffectSelection();
    clearNumberSelection();

    setRedoStack((stack) => {
      if (stack.length === 0) return stack;
      const [nextSnapshot, ...rest] = stack;
      setHistoryStack((history) => [...history, cloneAnnotations(annotationsRef.current)]);
      setAnnotationSnapshot(nextSnapshot);
      return rest;
    });
  }, [clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, setAnnotationSnapshot]);

  const commitSelectedTextMutation = useCallback(
    (ids: string[], updater: (annotation: TextAnnotation) => TextAnnotation | null) => {
      const uniqueIds = Array.from(new Set(ids.filter(Boolean)));
      if (uniqueIds.length === 0) {
        return false;
      }

      const selectedSet = new Set(uniqueIds);
      const primaryId = activeTextId && selectedSet.has(activeTextId) ? activeTextId : uniqueIds[uniqueIds.length - 1];
      let changed = false;
      let primaryAnnotation: TextAnnotation | null = null;

      const nextAnnotations: Annotation[] = [];
      for (const item of annotationsRef.current) {
        if (item.kind !== "text" || !selectedSet.has(item.id)) {
          nextAnnotations.push(item);
          continue;
        }

        const updated = updater(item);
        if (!updated) {
          changed = true;
          continue;
        }

        if (!areTextAnnotationsEqual(item, updated)) {
          changed = true;
        }

        nextAnnotations.push(updated);
        if (item.id === primaryId) {
          primaryAnnotation = updated;
        }
      }

      if (!changed) {
        return false;
      }

      commitAnnotations(nextAnnotations);
      const remainingIds = uniqueIds.filter((id) => findTextAnnotationById(nextAnnotations, id));
      if (remainingIds.length === 0) {
        clearTextSelection();
        return true;
      }

      const nextPrimaryId =
        primaryAnnotation?.id ??
        (activeTextId && remainingIds.includes(activeTextId) ? activeTextId : remainingIds[remainingIds.length - 1]);
      setTextSelection(remainingIds, nextPrimaryId, nextAnnotations, primaryAnnotation);
      return true;
    },
    [activeTextId, clearTextSelection, commitAnnotations, setTextSelection],
  );

  const commitSelectedEffectMutation = useCallback(
    (updater: (annotation: EffectAnnotation) => EffectAnnotation | null) => {
      const targetIds = getSelectedEffectIds();
      if (targetIds.length === 0) {
        return false;
      }

      const selectedSet = new Set(targetIds);
      const primaryId =
        selectedEffectId && selectedSet.has(selectedEffectId) ? selectedEffectId : targetIds[targetIds.length - 1];
      let changed = false;
      let primaryAnnotation: EffectAnnotation | null = null;
      const nextAnnotations: Annotation[] = [];

      for (const item of annotationsRef.current) {
        if (item.kind !== "effect" || !selectedSet.has(item.id)) {
          nextAnnotations.push(item);
          continue;
        }

        const updated = updater(item);
        if (!updated) {
          changed = true;
          continue;
        }

        if (!areEffectAnnotationsEqual(item, updated)) {
          changed = true;
        }

        nextAnnotations.push(updated);
        if (item.id === primaryId) {
          primaryAnnotation = updated;
        }
      }

      if (!changed) {
        return false;
      }

      commitAnnotations(nextAnnotations);
      const remainingIds = targetIds.filter((id) => findEffectAnnotationById(nextAnnotations, id));
      if (remainingIds.length === 0) {
        clearEffectSelection();
        return true;
      }

      const nextPrimaryId =
        primaryAnnotation?.id ??
        (selectedEffectId && remainingIds.includes(selectedEffectId) ? selectedEffectId : remainingIds[remainingIds.length - 1]);
      setEffectSelection(remainingIds, nextPrimaryId, nextAnnotations, primaryAnnotation);
      return true;
    },
    [clearEffectSelection, commitAnnotations, getSelectedEffectIds, selectedEffectId, setEffectSelection],
  );

  const commitSelectedNumberMutation = useCallback(
    (updater: (annotation: NumberAnnotation) => NumberAnnotation | null) => {
      const targetIds = getSelectedNumberIds();
      if (targetIds.length === 0) {
        return false;
      }

      const selectedSet = new Set(targetIds);
      const primaryId =
        selectedNumberId && selectedSet.has(selectedNumberId) ? selectedNumberId : targetIds[targetIds.length - 1];
      let changed = false;
      let primaryAnnotation: NumberAnnotation | null = null;
      const nextAnnotations: Annotation[] = [];

      for (const item of annotationsRef.current) {
        if (item.kind !== "number" || !selectedSet.has(item.id)) {
          nextAnnotations.push(item);
          continue;
        }

        const updated = updater(item);
        if (!updated) {
          changed = true;
          continue;
        }

        if (!areNumberAnnotationsEqual(item, updated)) {
          changed = true;
        }

        nextAnnotations.push(updated);
        if (item.id === primaryId) {
          primaryAnnotation = updated;
        }
      }

      if (!changed) {
        return false;
      }

      commitAnnotations(nextAnnotations);
      const remainingIds = targetIds.filter((id) => findNumberAnnotationById(nextAnnotations, id));
      if (remainingIds.length === 0) {
        clearNumberSelection();
        return true;
      }

      const nextPrimaryId =
        primaryAnnotation?.id ??
        (selectedNumberId && remainingIds.includes(selectedNumberId) ? selectedNumberId : remainingIds[remainingIds.length - 1]);
      setNumberSelection(remainingIds, nextPrimaryId, nextAnnotations, primaryAnnotation);
      return true;
    },
    [clearNumberSelection, commitAnnotations, getSelectedNumberIds, selectedNumberId, setNumberSelection],
  );

  const commitSelectedShapeMutation = useCallback(
    (updater: (annotation: ShapeAnnotation) => ShapeAnnotation | null) => {
      const targetIds = getSelectedShapeIds();
      if (targetIds.length === 0) {
        return false;
      }

      const selectedSet = new Set(targetIds);
      const primaryId = selectedShapeId && selectedSet.has(selectedShapeId) ? selectedShapeId : targetIds[targetIds.length - 1];
      let changed = false;
      let primaryAnnotation: ShapeAnnotation | null = null;
      const nextAnnotations: Annotation[] = [];

      for (const item of annotationsRef.current) {
        if (
          (item.kind !== "line" && item.kind !== "rect" && item.kind !== "ellipse" && item.kind !== "arrow") ||
          !selectedSet.has(item.id)
        ) {
          nextAnnotations.push(item);
          continue;
        }

        const updated = updater(item);
        if (!updated) {
          changed = true;
          continue;
        }

        if (!areShapeAnnotationsEqual(item, updated)) {
          changed = true;
        }

        if (updated.id === primaryId) {
          primaryAnnotation = updated;
        }
        nextAnnotations.push(updated);
      }

      if (!changed) {
        return false;
      }

      const remainingIds = targetIds.filter((id) =>
        nextAnnotations.some(
          (annotation) =>
            (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") &&
            annotation.id === id,
        ),
      );

      commitAnnotations(nextAnnotations);
      if (remainingIds.length === 0) {
        clearShapeSelection();
        return true;
      }

      const nextPrimaryId =
        primaryAnnotation?.id ?? (selectedShapeId && remainingIds.includes(selectedShapeId) ? selectedShapeId : remainingIds[remainingIds.length - 1]);
      setShapeSelection(
        remainingIds,
        nextPrimaryId,
        nextAnnotations,
        nextPrimaryId ? findShapeAnnotationById(nextAnnotations, nextPrimaryId) : null,
      );
      return true;
    },
    [clearShapeSelection, commitAnnotations, getSelectedShapeIds, selectedShapeId, setShapeSelection],
  );

  const commitSelectedPenMutation = useCallback(
    (updater: (annotation: PenAnnotation) => PenAnnotation | null) => {
      const targetIds = getSelectedPenIds();
      if (targetIds.length === 0) {
        return false;
      }

      const selectedSet = new Set(targetIds);
      const primaryId =
        selectedPenId && selectedSet.has(selectedPenId) ? selectedPenId : targetIds[targetIds.length - 1];
      let changed = false;
      let primaryAnnotation: PenAnnotation | null = null;
      const nextAnnotations: Annotation[] = [];

      for (const item of annotationsRef.current) {
        if (item.kind !== "pen" || !selectedSet.has(item.id)) {
          nextAnnotations.push(item);
          continue;
        }

        const updated = updater(item);
        if (!updated) {
          changed = true;
          continue;
        }

        if (!arePenAnnotationsEqual(item, updated)) {
          changed = true;
        }

        nextAnnotations.push(updated);
        if (item.id === primaryId) {
          primaryAnnotation = updated;
        }
      }

      if (!changed) {
        return false;
      }

      commitAnnotations(nextAnnotations);
      const remainingIds = targetIds.filter((id) => findPenAnnotationById(nextAnnotations, id));
      if (remainingIds.length === 0) {
        clearPenSelection();
        return true;
      }

      const nextPrimaryId =
        primaryAnnotation?.id ??
        (selectedPenId && remainingIds.includes(selectedPenId) ? selectedPenId : remainingIds[remainingIds.length - 1]);
      setPenSelection(remainingIds, nextPrimaryId, nextAnnotations, primaryAnnotation);
      return true;
    },
    [clearPenSelection, commitAnnotations, getSelectedPenIds, selectedPenId, setPenSelection],
  );

  const applyStrokeWidthValue = useCallback(
    (value: number) => {
      const nextWidth = clampNumber(value, 1, 18);
      setStrokeWidth(nextWidth);

      if (!selectedShapeAnnotation) {
        if (!selectedPenAnnotation) {
          return;
        }

        commitSelectedPenMutation((annotation) => {
          if (annotation.strokeWidth === nextWidth) {
            return annotation;
          }
          return { ...annotation, strokeWidth: nextWidth };
        });
        return;
      }

      commitSelectedShapeMutation((annotation) => {
        if (annotation.strokeWidth === nextWidth) {
          return annotation;
        }
        return { ...annotation, strokeWidth: nextWidth };
      });
    },
    [commitSelectedPenMutation, commitSelectedShapeMutation, selectedPenAnnotation, selectedShapeAnnotation],
  );

  const applyFontSize = useCallback(
    (value: number) => {
      const nextSize = clampNumber(value, 10, 64);
      setFontSize(nextSize);

      if (textEditorStateRef.current) {
        updateTextEditor((current) => ({ ...current, fontSize: nextSize }));
        return;
      }

      const targetIds = getSelectedTextIds();
      if ((tool === "text" || tool === "select") && targetIds.length > 0) {
        commitSelectedTextMutation(targetIds, (annotation) => {
          if (annotation.fontSize === nextSize) {
            return annotation;
          }
          const updated = { ...annotation, fontSize: nextSize };
          return selection ? fitTextAnnotationToSelection(updated, selection) : updated;
        });
        return;
      }

      if (getSelectedNumberIds().length === 0) {
        return;
      }

      commitSelectedNumberMutation((annotation) => {
        if (annotation.size === nextSize) {
          return annotation;
        }
        const updated = { ...annotation, size: nextSize };
        return selection ? clampNumberAnnotationToSelection(updated, selection) : updated;
      });
    },
    [commitSelectedNumberMutation, commitSelectedTextMutation, getSelectedNumberIds, getSelectedTextIds, selection, tool, updateTextEditor],
  );

  const applyTextStyle = useCallback(
    (nextStyle: TextStyleKind) => {
      setTextStyle(nextStyle);

      if (textEditorStateRef.current) {
        updateTextEditor((current) => ({ ...current, style: nextStyle }));
        return;
      }

      if (tool !== "text" && tool !== "select") return;
      const targetIds = getSelectedTextIds();
      if (targetIds.length === 0) return;

      commitSelectedTextMutation(targetIds, (annotation) => {
        if (annotation.style === nextStyle) {
          return annotation;
        }
        const updated = { ...annotation, style: nextStyle };
        return selection ? fitTextAnnotationToSelection(updated, selection) : updated;
      });
    },
    [commitSelectedTextMutation, getSelectedTextIds, selection, tool, updateTextEditor],
  );

  const applyTextRotation = useCallback(
    (value: number) => {
      const nextRotation = clampNumber(value, -180, 180);
      setTextRotation(nextRotation);

      if (textEditorStateRef.current) {
        updateTextEditor((current) => ({ ...current, rotation: nextRotation }));
        return;
      }

      const targetIds = getSelectedTextIds();
      if (targetIds.length === 0) return;

      commitSelectedTextMutation(targetIds, (annotation) => {
        if (annotation.rotation === nextRotation) {
          return annotation;
        }
        const updated = { ...annotation, rotation: nextRotation };
        return selection ? fitTextAnnotationToSelection(updated, selection) : updated;
      });
    },
    [commitSelectedTextMutation, getSelectedTextIds, selection, updateTextEditor],
  );

  const applyTextOpacityValue = useCallback(
    (value: number) => {
      const nextOpacityPercent = clampNumber(value, 10, 100);
      const nextOpacity = nextOpacityPercent / 100;
      setTextOpacity(nextOpacityPercent);

      if (textEditorStateRef.current) {
        updateTextEditor((current) => ({ ...current, opacity: nextOpacity }));
        return;
      }

      const targetIds = getSelectedTextIds();
      if (targetIds.length === 0) return;

      commitSelectedTextMutation(targetIds, (annotation) => {
        if (Math.abs(annotation.opacity - nextOpacity) < 0.001) {
          return annotation;
        }
        return { ...annotation, opacity: nextOpacity };
      });
    },
    [commitSelectedTextMutation, getSelectedTextIds, updateTextEditor],
  );

  const applyEffectIntensity = useCallback(
    (effect: EffectKind, value: number) => {
      const nextIntensity =
        effect === "mosaic"
          ? clampNumber(value, 4, 48)
          : clampNumber(value, 2, 24);

      if (effect === "mosaic") {
        setMosaicSize(nextIntensity);
      } else {
        setBlurRadius(nextIntensity);
      }

      commitSelectedEffectMutation((annotation) => {
        if (annotation.effect !== effect || Math.abs(annotation.intensity - nextIntensity) < 0.001) {
          return annotation;
        }
        return {
          ...annotation,
          intensity: nextIntensity,
        };
      });
    },
    [commitSelectedEffectMutation],
  );

  const deleteSelectedEffect = useCallback(() => {
    commitSelectedEffectMutation(() => null);
  }, [commitSelectedEffectMutation]);

  const deleteSelectedShape = useCallback(() => {
    commitSelectedShapeMutation(() => null);
  }, [commitSelectedShapeMutation]);

  const deleteSelectedPen = useCallback(() => {
    commitSelectedPenMutation(() => null);
  }, [commitSelectedPenMutation]);

  const deleteSelectedNumber = useCallback(() => {
    commitSelectedNumberMutation(() => null);
  }, [commitSelectedNumberMutation]);

  const nudgeSelectedTexts = useCallback(
    (dx: number, dy: number) => {
      if (!selection) return;
      const targetIds = getSelectedTextIds();
      if (targetIds.length === 0) return;

      const selectedAnnotations = targetIds
        .map((id) => findTextAnnotationById(annotationsRef.current, id))
        .filter((annotation): annotation is TextAnnotation => annotation !== null);

      if (selectedAnnotations.length === 0) {
        return;
      }

      const groupBounds = resolveTextGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) return;

      commitSelectedTextMutation(targetIds, (annotation) => ({
        ...annotation,
        point: {
          x: annotation.point.x + delta.x,
          y: annotation.point.y + delta.y,
        },
      }));
    },
    [commitSelectedTextMutation, getSelectedTextIds, selection],
  );

  const nudgeSelectedEffect = useCallback(
    (dx: number, dy: number) => {
      if (!selection) {
        return;
      }

      const targetIds = getSelectedEffectIds();
      if (targetIds.length === 0) {
        return;
      }

      const selectedAnnotations = targetIds
        .map((id) => findEffectAnnotationById(annotationsRef.current, id))
        .filter((annotation): annotation is EffectAnnotation => annotation !== null);
      if (selectedAnnotations.length === 0) {
        return;
      }

      const groupBounds = resolveEffectGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) {
        return;
      }

      commitSelectedEffectMutation((annotation) => {
        const bounds = resolveEffectAnnotationBounds(annotation);
        const nextBounds = offsetRect(bounds, delta);

        if (areSelectionRectsEqual(bounds, nextBounds)) {
          return annotation;
        }

        return createEffectAnnotationWithBounds(annotation, nextBounds);
      });
    },
    [commitSelectedEffectMutation, getSelectedEffectIds, selection],
  );

  const nudgeSelectedShape = useCallback(
    (dx: number, dy: number) => {
      if (!selection) {
        return;
      }

      const selectedAnnotations = getSelectedShapeIds()
        .map((id) => findShapeAnnotationById(annotationsRef.current, id))
        .filter((annotation): annotation is ShapeAnnotation => annotation !== null);
      if (selectedAnnotations.length === 0) {
        return;
      }

      const groupBounds = resolveShapeGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) {
        return;
      }

      commitSelectedShapeMutation((annotation) => {
        const nextAnnotation = offsetShapeAnnotation(annotation, delta);
        if (areShapeAnnotationsEqual(annotation, nextAnnotation)) {
          return annotation;
        }
        return nextAnnotation;
      });
    },
    [commitSelectedShapeMutation, getSelectedShapeIds, selection],
  );

  const nudgeSelectedPen = useCallback(
    (dx: number, dy: number) => {
      if (!selection) {
        return;
      }

      const selectedAnnotations = getSelectedPenIds()
        .map((id) => findPenAnnotationById(annotationsRef.current, id))
        .filter((annotation): annotation is PenAnnotation => annotation !== null);
      if (selectedAnnotations.length === 0) {
        return;
      }

      const groupBounds = resolvePenGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) {
        return;
      }

      commitSelectedPenMutation((annotation) => {
        const nextAnnotation = offsetPenAnnotation(annotation, delta);
        if (arePenAnnotationsEqual(annotation, nextAnnotation)) {
          return annotation;
        }
        return nextAnnotation;
      });
    },
    [commitSelectedPenMutation, getSelectedPenIds, selection],
  );

  const nudgeSelectedNumber = useCallback(
    (dx: number, dy: number) => {
      if (!selection) {
        return;
      }

      const targetIds = getSelectedNumberIds();
      if (targetIds.length === 0) {
        return;
      }

      const selectedAnnotations = targetIds
        .map((id) => findNumberAnnotationById(annotationsRef.current, id))
        .filter((annotation): annotation is NumberAnnotation => annotation !== null);
      if (selectedAnnotations.length === 0) {
        return;
      }

      const groupBounds = resolveNumberGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) {
        return;
      }

      commitSelectedNumberMutation((annotation) => {
        const nextAnnotation = {
          ...annotation,
          point: {
            x: annotation.point.x + delta.x,
            y: annotation.point.y + delta.y,
          },
        };

        if (arePointsEqual(annotation.point, nextAnnotation.point)) {
          return annotation;
        }

        return nextAnnotation;
      });
    },
    [commitSelectedNumberMutation, getSelectedNumberIds, selection],
  );

  const deleteSelectedTexts = useCallback(() => {
    const targetIds = getSelectedTextIds();
    if (targetIds.length === 0) return;

    const selectedSet = new Set(targetIds);
    const nextAnnotations = annotationsRef.current.filter(
      (annotation) => annotation.kind !== "text" || !selectedSet.has(annotation.id),
    );

    if (nextAnnotations.length === annotationsRef.current.length) {
      return;
    }

    commitAnnotations(nextAnnotations);
    clearTextSelection();
  }, [clearTextSelection, commitAnnotations, getSelectedTextIds]);

  const moveSelectedTextLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const targetIds = getSelectedTextIds();
      if (targetIds.length === 0) return;

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return;
      }

      commitAnnotations(nextAnnotations);
      const primaryId =
        activeTextId && targetIds.includes(activeTextId) ? activeTextId : targetIds[targetIds.length - 1];
      setTextSelection(targetIds, primaryId, nextAnnotations);
    },
    [activeTextId, commitAnnotations, getSelectedTextIds, setTextSelection],
  );

  const moveSelectedNumberLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const targetIds = getSelectedNumberIds();
      if (targetIds.length === 0) {
        return;
      }

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return;
      }

      commitAnnotations(nextAnnotations);
      const primaryId =
        selectedNumberId && targetIds.includes(selectedNumberId) ? selectedNumberId : targetIds[targetIds.length - 1];
      setNumberSelection(targetIds, primaryId, nextAnnotations);
    },
    [commitAnnotations, getSelectedNumberIds, selectedNumberId, setNumberSelection],
  );

  const moveSelectedEffectLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const targetIds = getSelectedEffectIds();
      if (targetIds.length === 0) {
        return;
      }

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return;
      }

      commitAnnotations(nextAnnotations);
      const primaryId =
        selectedEffectId && targetIds.includes(selectedEffectId) ? selectedEffectId : targetIds[targetIds.length - 1];
      setEffectSelection(targetIds, primaryId, nextAnnotations);
    },
    [commitAnnotations, getSelectedEffectIds, selectedEffectId, setEffectSelection],
  );

  const moveSelectedShapeLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const targetIds = getSelectedShapeIds();
      if (targetIds.length === 0) {
        return;
      }

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return;
      }

      commitAnnotations(nextAnnotations);
      const primaryId =
        selectedShapeId && targetIds.includes(selectedShapeId) ? selectedShapeId : targetIds[targetIds.length - 1];
      setShapeSelection(
        targetIds,
        primaryId,
        nextAnnotations,
        primaryId ? findShapeAnnotationById(nextAnnotations, primaryId) : null,
      );
    },
    [commitAnnotations, getSelectedShapeIds, selectedShapeId, setShapeSelection],
  );

  const moveSelectedPenLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const targetIds = getSelectedPenIds();
      if (targetIds.length === 0) {
        return;
      }

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return;
      }

      commitAnnotations(nextAnnotations);
      const primaryId =
        selectedPenId && targetIds.includes(selectedPenId) ? selectedPenId : targetIds[targetIds.length - 1];
      setPenSelection(
        targetIds,
        primaryId,
        nextAnnotations,
        primaryId ? findPenAnnotationById(nextAnnotations, primaryId) : null,
      );
    },
    [commitAnnotations, getSelectedPenIds, selectedPenId, setPenSelection],
  );

  const selectAllObjects = useCallback(() => {
    const nextTextAnnotations = annotationsRef.current.filter(
      (annotation): annotation is TextAnnotation => annotation.kind === "text",
    );
    const nextShapeAnnotations = annotationsRef.current.filter(
      (annotation): annotation is ShapeAnnotation =>
        annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow",
    );
    const nextPenAnnotations = annotationsRef.current.filter(
      (annotation): annotation is PenAnnotation => annotation.kind === "pen",
    );
    const nextNumberAnnotations = annotationsRef.current.filter(
      (annotation): annotation is NumberAnnotation => annotation.kind === "number",
    );
    const nextEffectAnnotations = annotationsRef.current.filter(
      (annotation): annotation is EffectAnnotation => annotation.kind === "effect",
    );

    const total =
      nextTextAnnotations.length +
      nextShapeAnnotations.length +
      nextPenAnnotations.length +
      nextNumberAnnotations.length +
      nextEffectAnnotations.length;
    if (total === 0) {
      return false;
    }

    if (nextTextAnnotations.length > 0) {
      const primary = nextTextAnnotations[nextTextAnnotations.length - 1] ?? null;
      setTextSelection(
        nextTextAnnotations.map((annotation) => annotation.id),
        primary?.id ?? null,
        annotationsRef.current,
        primary,
      );
    } else {
      clearTextSelection();
    }

    if (nextShapeAnnotations.length > 0) {
      const primary = nextShapeAnnotations[nextShapeAnnotations.length - 1] ?? null;
      setShapeSelection(
        nextShapeAnnotations.map((annotation) => annotation.id),
        primary?.id ?? null,
        annotationsRef.current,
        primary,
      );
    } else {
      clearShapeSelection();
    }

    if (nextPenAnnotations.length > 0) {
      const primary = nextPenAnnotations[nextPenAnnotations.length - 1] ?? null;
      setPenSelection(
        nextPenAnnotations.map((annotation) => annotation.id),
        primary?.id ?? null,
        annotationsRef.current,
        primary,
      );
    } else {
      clearPenSelection();
    }

    if (nextNumberAnnotations.length > 0) {
      const primary = nextNumberAnnotations[nextNumberAnnotations.length - 1] ?? null;
      setNumberSelection(
        nextNumberAnnotations.map((annotation) => annotation.id),
        primary?.id ?? null,
        annotationsRef.current,
        primary,
      );
    } else {
      clearNumberSelection();
    }

    if (nextEffectAnnotations.length > 0) {
      const primary = nextEffectAnnotations[nextEffectAnnotations.length - 1] ?? null;
      setEffectSelection(
        nextEffectAnnotations.map((annotation) => annotation.id),
        primary?.id ?? null,
        annotationsRef.current,
        primary,
      );
    } else {
      clearEffectSelection();
    }

    return true;
  }, [clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, setEffectSelection, setNumberSelection, setPenSelection, setShapeSelection, setTextSelection]);

  const getSelectedTextAnnotations = useCallback(() => {
    const selectedSet = new Set(getSelectedTextIds());
    if (selectedSet.size === 0) {
      return [];
    }

    return annotationsRef.current.filter(
      (annotation): annotation is TextAnnotation => annotation.kind === "text" && selectedSet.has(annotation.id),
    );
  }, [getSelectedTextIds]);

  const getSelectedShapeAnnotations = useCallback(() => {
    const selectedSet = new Set(getSelectedShapeIds());
    if (selectedSet.size === 0) {
      return [];
    }

    return annotationsRef.current.filter(
      (annotation): annotation is ShapeAnnotation =>
        (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") &&
        selectedSet.has(annotation.id),
    );
  }, [getSelectedShapeIds]);

  const getSelectedNumberAnnotations = useCallback(() => {
    const selectedSet = new Set(getSelectedNumberIds());
    if (selectedSet.size === 0) {
      return [];
    }

    return annotationsRef.current.filter(
      (annotation): annotation is NumberAnnotation => annotation.kind === "number" && selectedSet.has(annotation.id),
    );
  }, [getSelectedNumberIds]);

  const getSelectedPenAnnotations = useCallback(() => {
    const selectedSet = new Set(getSelectedPenIds());
    if (selectedSet.size === 0) {
      return [];
    }

    return annotationsRef.current.filter(
      (annotation): annotation is PenAnnotation => annotation.kind === "pen" && selectedSet.has(annotation.id),
    );
  }, [getSelectedPenIds]);

  const getSelectedEffectAnnotations = useCallback(() => {
    const selectedSet = new Set(getSelectedEffectIds());
    if (selectedSet.size === 0) {
      return [];
    }

    return annotationsRef.current.filter(
      (annotation): annotation is EffectAnnotation => annotation.kind === "effect" && selectedSet.has(annotation.id),
    );
  }, [getSelectedEffectIds]);

  const getSelectedObjectAnnotations = useCallback(
    (): ObjectSelectionAnnotation[] => {
      const selectedIds = new Set([
        ...getSelectedTextIds(),
        ...getSelectedShapeIds(),
        ...getSelectedPenIds(),
        ...getSelectedNumberIds(),
        ...getSelectedEffectIds(),
      ]);

      return annotationsRef.current.filter(
        (annotation): annotation is ObjectSelectionAnnotation => annotation.kind !== "fill" && selectedIds.has(annotation.id),
      );
    },
    [
      getSelectedEffectIds,
      getSelectedNumberIds,
      getSelectedPenIds,
      getSelectedShapeIds,
      getSelectedTextIds,
    ],
  );

  const getSelectedObjectBuckets = useCallback(
    (): ObjectSelectionBuckets => ({
      text: getSelectedTextIds(),
      shape: getSelectedShapeIds(),
      pen: getSelectedPenIds(),
      number: getSelectedNumberIds(),
      effect: getSelectedEffectIds(),
    }),
    [getSelectedEffectIds, getSelectedNumberIds, getSelectedPenIds, getSelectedShapeIds, getSelectedTextIds],
  );

  const restoreObjectSelections = useCallback(
    (buckets: ObjectSelectionBuckets, sourceAnnotations: Annotation[] = annotationsRef.current) => {
      const nextTextIds = buckets.text.filter((id) => findTextAnnotationById(sourceAnnotations, id) !== null);
      const nextShapeIds = buckets.shape.filter((id) => findShapeAnnotationById(sourceAnnotations, id) !== null);
      const nextPenIds = buckets.pen.filter((id) => findPenAnnotationById(sourceAnnotations, id) !== null);
      const nextNumberIds = buckets.number.filter((id) => findNumberAnnotationById(sourceAnnotations, id) !== null);
      const nextEffectIds = buckets.effect.filter((id) => findEffectAnnotationById(sourceAnnotations, id) !== null);

      if (nextTextIds.length > 0) {
        const primaryId = activeTextId && nextTextIds.includes(activeTextId) ? activeTextId : nextTextIds[nextTextIds.length - 1];
        setTextSelection(
          nextTextIds,
          primaryId,
          sourceAnnotations,
          primaryId ? findTextAnnotationById(sourceAnnotations, primaryId) : null,
        );
      } else {
        clearTextSelection();
      }

      if (nextShapeIds.length > 0) {
        const primaryId =
          selectedShapeId && nextShapeIds.includes(selectedShapeId) ? selectedShapeId : nextShapeIds[nextShapeIds.length - 1];
        setShapeSelection(
          nextShapeIds,
          primaryId,
          sourceAnnotations,
          primaryId ? findShapeAnnotationById(sourceAnnotations, primaryId) : null,
        );
      } else {
        clearShapeSelection();
      }

      if (nextPenIds.length > 0) {
        const primaryId = selectedPenId && nextPenIds.includes(selectedPenId) ? selectedPenId : nextPenIds[nextPenIds.length - 1];
        setPenSelection(
          nextPenIds,
          primaryId,
          sourceAnnotations,
          primaryId ? findPenAnnotationById(sourceAnnotations, primaryId) : null,
        );
      } else {
        clearPenSelection();
      }

      if (nextNumberIds.length > 0) {
        const primaryId =
          selectedNumberId && nextNumberIds.includes(selectedNumberId) ? selectedNumberId : nextNumberIds[nextNumberIds.length - 1];
        setNumberSelection(
          nextNumberIds,
          primaryId,
          sourceAnnotations,
          primaryId ? findNumberAnnotationById(sourceAnnotations, primaryId) : null,
        );
      } else {
        clearNumberSelection();
      }

      if (nextEffectIds.length > 0) {
        const primaryId =
          selectedEffectId && nextEffectIds.includes(selectedEffectId) ? selectedEffectId : nextEffectIds[nextEffectIds.length - 1];
        setEffectSelection(
          nextEffectIds,
          primaryId,
          sourceAnnotations,
          primaryId ? findEffectAnnotationById(sourceAnnotations, primaryId) : null,
        );
      } else {
        clearEffectSelection();
      }
    },
    [
      activeTextId,
      clearEffectSelection,
      clearNumberSelection,
      clearPenSelection,
      clearShapeSelection,
      clearTextSelection,
      selectedEffectId,
      selectedNumberId,
      selectedPenId,
      selectedShapeId,
      setEffectSelection,
      setNumberSelection,
      setPenSelection,
      setShapeSelection,
      setTextSelection,
    ],
  );

  const nudgeSelectedMixedObjects = useCallback(
    (dx: number, dy: number) => {
      if (!selection) {
        return false;
      }

      const buckets = getSelectedObjectBuckets();
      const targetIds = Array.from(
        new Set([...buckets.text, ...buckets.shape, ...buckets.pen, ...buckets.number, ...buckets.effect]),
      );
      if (targetIds.length === 0) {
        return false;
      }

      const selectedAnnotations = getSelectedObjectAnnotations();
      if (selectedAnnotations.length === 0) {
        return false;
      }

      const groupBounds = resolveObjectSelectionGroupBounds(selectedAnnotations);
      const delta = clampGroupDeltaToSelection({ x: dx, y: dy }, groupBounds, selection);
      if (delta.x === 0 && delta.y === 0) {
        return false;
      }

      const selectedSet = new Set(targetIds);
      const nextAnnotations = annotationsRef.current.map((annotation) => {
        if (annotation.kind === "fill" || !selectedSet.has(annotation.id)) {
          return annotation;
        }
        return offsetObjectSelectionAnnotation(annotation, delta);
      });

      commitAnnotations(nextAnnotations);
      restoreObjectSelections(buckets, nextAnnotations);
      return true;
    },
    [commitAnnotations, getSelectedObjectAnnotations, getSelectedObjectBuckets, restoreObjectSelections, selection],
  );

  const deleteSelectedMixedObjects = useCallback(() => {
    const buckets = getSelectedObjectBuckets();
    const selectedIds = Array.from(
      new Set([...buckets.text, ...buckets.shape, ...buckets.pen, ...buckets.number, ...buckets.effect]),
    );
    if (selectedIds.length === 0) {
      return false;
    }

    const selectedSet = new Set(selectedIds);
    const nextAnnotations = annotationsRef.current.filter((annotation) => !selectedSet.has(annotation.id));
    if (nextAnnotations.length === annotationsRef.current.length) {
      return false;
    }

    commitAnnotations(nextAnnotations);
    clearTextSelection();
    clearShapeSelection();
    clearPenSelection();
    clearNumberSelection();
    clearEffectSelection();
    return true;
  }, [clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, commitAnnotations, getSelectedObjectBuckets]);

  const moveSelectedMixedLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const buckets = getSelectedObjectBuckets();
      const targetIds = Array.from(
        new Set([...buckets.text, ...buckets.shape, ...buckets.pen, ...buckets.number, ...buckets.effect]),
      );
      if (targetIds.length === 0) {
        return false;
      }

      const nextAnnotations = moveAnnotationLayer(annotationsRef.current, targetIds, direction);
      if (!nextAnnotations) {
        return false;
      }

      commitAnnotations(nextAnnotations);
      restoreObjectSelections(buckets, nextAnnotations);
      return true;
    },
    [commitAnnotations, getSelectedObjectBuckets, restoreObjectSelections],
  );

  const moveSelectedAnnotationLayer = useCallback(
    (direction: "forward" | "backward" | "front" | "back") => {
      const buckets = getSelectedObjectBuckets();
      const selectedFamilyCountForMove = [
        buckets.text.length > 0,
        buckets.shape.length > 0,
        buckets.pen.length > 0,
        buckets.number.length > 0,
        buckets.effect.length > 0,
      ].filter(Boolean).length;

      if (selectedFamilyCountForMove > 1) {
        return moveSelectedMixedLayer(direction);
      }

      if (buckets.text.length > 0) {
        moveSelectedTextLayer(direction);
        return true;
      }

      if (buckets.shape.length > 0) {
        moveSelectedShapeLayer(direction);
        return true;
      }

      if (buckets.pen.length > 0) {
        moveSelectedPenLayer(direction);
        return true;
      }

      if (buckets.number.length > 0) {
        moveSelectedNumberLayer(direction);
        return true;
      }

      if (buckets.effect.length > 0) {
        moveSelectedEffectLayer(direction);
        return true;
      }

      return false;
    },
    [getSelectedObjectBuckets, moveSelectedEffectLayer, moveSelectedMixedLayer, moveSelectedNumberLayer, moveSelectedPenLayer, moveSelectedShapeLayer, moveSelectedTextLayer],
  );

  const splitObjectAnnotationsToBuckets = useCallback(
    (annotations: ObjectSelectionAnnotation[]): ObjectSelectionBuckets => ({
      text: annotations.filter((annotation): annotation is TextAnnotation => annotation.kind === "text").map((annotation) => annotation.id),
      shape: annotations
        .filter(
          (annotation): annotation is ShapeAnnotation =>
            annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow",
        )
        .map((annotation) => annotation.id),
      pen: annotations.filter((annotation): annotation is PenAnnotation => annotation.kind === "pen").map((annotation) => annotation.id),
      number: annotations.filter((annotation): annotation is NumberAnnotation => annotation.kind === "number").map((annotation) => annotation.id),
      effect: annotations.filter((annotation): annotation is EffectAnnotation => annotation.kind === "effect").map((annotation) => annotation.id),
    }),
    [],
  );

  const writeMixedClipboard = useCallback((annotationsToCopy: ObjectSelectionAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      mixedClipboardRef.current = null;
      if (objectClipboardKindRef.current === "mixed") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    mixedClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as ObjectSelectionAnnotation),
      groupBounds: resolveObjectSelectionGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "mixed";
  }, []);

  const copySelectedMixedObjects = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedObjectAnnotations();
      const buckets = splitObjectAnnotationsToBuckets(selectedAnnotations);
      const selectedFamilyCountForCopy = [
        buckets.text.length > 0,
        buckets.shape.length > 0,
        buckets.pen.length > 0,
        buckets.number.length > 0,
        buckets.effect.length > 0,
      ].filter(Boolean).length;

      if (selectedAnnotations.length === 0 || selectedFamilyCountForCopy < 2) {
        if (showFeedback) {
          message.warning("请先建立跨家族混选");
        }
        return false;
      }

      writeMixedClipboard(selectedAnnotations);
      if (showFeedback) {
        message.success(`已复制 ${selectedAnnotations.length} 个混选对象`);
      }
      return true;
    },
    [getSelectedObjectAnnotations, message, splitObjectAnnotationsToBuckets, writeMixedClipboard],
  );

  const writeTextClipboard = useCallback((annotationsToCopy: TextAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      textClipboardRef.current = null;
      if (objectClipboardKindRef.current === "text") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    textClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as TextAnnotation),
      groupBounds: resolveTextGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "text";
  }, []);

  const writeShapeClipboard = useCallback((annotationsToCopy: ShapeAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      shapeClipboardRef.current = null;
      if (objectClipboardKindRef.current === "shape") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    shapeClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as ShapeAnnotation),
      groupBounds: resolveShapeGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "shape";
  }, []);

  const copySelectedShape = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedShapeAnnotations();
      if (selectedAnnotations.length === 0) {
        if (showFeedback) {
          message.warning("请先选中图形对象");
        }
        return false;
      }

      writeShapeClipboard(selectedAnnotations);
      if (showFeedback) {
        if (selectedAnnotations.length === 1) {
          message.success(`已复制${getShapeKindLabel(selectedAnnotations[0].kind)}`);
        } else {
          message.success(`已复制 ${selectedAnnotations.length} 个图形对象`);
        }
      }
      return true;
    },
    [getSelectedShapeAnnotations, message, writeShapeClipboard],
  );

  const copySelectedTexts = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedTextAnnotations();
      if (selectedAnnotations.length === 0) {
        if (showFeedback) {
          message.warning("请先选中文字对象");
        }
        return false;
      }

      writeTextClipboard(selectedAnnotations);
      if (showFeedback) {
        message.success(`已复制 ${selectedAnnotations.length} 个文字对象`);
      }
      return true;
    },
    [getSelectedTextAnnotations, message, writeTextClipboard],
  );

  const writePenClipboard = useCallback((annotationsToCopy: PenAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      penClipboardRef.current = null;
      if (objectClipboardKindRef.current === "pen") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    penClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as PenAnnotation),
      groupBounds: resolvePenGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "pen";
  }, []);

  const copySelectedPen = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedPenAnnotations();
      if (selectedAnnotations.length === 0) {
        if (showFeedback) {
          message.warning("请先选中画笔对象");
        }
        return false;
      }

      writePenClipboard(selectedAnnotations);
      if (showFeedback) {
        if (selectedAnnotations.length === 1) {
          message.success("已复制画笔路径");
        } else {
          message.success(`已复制 ${selectedAnnotations.length} 条画笔路径`);
        }
      }
      return true;
    },
    [getSelectedPenAnnotations, message, writePenClipboard],
  );

  const pasteShapeClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: ShapeAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let requestedOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = shapeClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的图形对象");
          return false;
        }

        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as ShapeAnnotation);
        sourceBounds = clipboard.groupBounds;
        requestedOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedShapeAnnotations();
        if (selectedAnnotations.length === 0) {
          message.warning("请先选中图形对象");
          return false;
        }

        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as ShapeAnnotation);
        sourceBounds = resolveShapeGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(requestedOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) =>
        offsetShapeAnnotation(
          {
            ...annotation,
            id: crypto.randomUUID(),
          },
          offset,
        ),
      );

      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      clearTextSelection();
      clearPenSelection();
      clearEffectSelection();
      clearNumberSelection();
      objectClipboardKindRef.current = "shape";

      const primaryShape = duplicatedItems[duplicatedItems.length - 1];
      if (!primaryShape) {
        return false;
      }
      setShapeSelection(
        duplicatedItems.map((annotation) => annotation.id),
        primaryShape.id,
        nextAnnotations,
        primaryShape,
      );

      if (mode === "duplicate") {
        if (duplicatedItems.length === 1) {
          message.success(`已重复${getShapeKindLabel(primaryShape.kind)}`);
        } else {
          message.success(`已重复 ${duplicatedItems.length} 个图形对象`);
        }
      } else if (duplicatedItems.length === 1) {
        message.success(`已粘贴${getShapeKindLabel(primaryShape.kind)}`);
      } else {
        message.success(`已粘贴 ${duplicatedItems.length} 个图形对象`);
      }
      return true;
    },
    [
      clearEffectSelection,
      clearNumberSelection,
      clearPenSelection,
      clearTextSelection,
      commitAnnotations,
      getSelectedShapeAnnotations,
      message,
      selection,
      setShapeSelection,
    ],
  );

  const pasteTextClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: TextAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let rawOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = textClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的文字对象");
          return false;
        }
        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as TextAnnotation);
        sourceBounds = clipboard.groupBounds;
        rawOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedTextAnnotations();
        if (selectedAnnotations.length === 0) {
          message.warning("请先选中文字对象");
          return false;
        }
        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as TextAnnotation);
        sourceBounds = resolveTextGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(rawOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) => ({
        ...annotation,
        id: crypto.randomUUID(),
        point: {
          x: annotation.point.x + offset.x,
          y: annotation.point.y + offset.y,
        },
      }));

      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      setTextSelection(
        duplicatedItems.map((annotation) => annotation.id),
        duplicatedItems[duplicatedItems.length - 1]?.id ?? null,
        nextAnnotations,
        duplicatedItems[duplicatedItems.length - 1] ?? null,
      );
      objectClipboardKindRef.current = "text";

      if (mode === "duplicate") {
        message.success(`已重复 ${duplicatedItems.length} 个文字对象`);
      } else {
        message.success(`已粘贴 ${duplicatedItems.length} 个文字对象`);
      }
      return true;
    },
    [commitAnnotations, getSelectedTextAnnotations, message, selection, setTextSelection],
  );

  const pastePenClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: PenAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let requestedOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = penClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的画笔对象");
          return false;
        }

        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as PenAnnotation);
        sourceBounds = clipboard.groupBounds;
        requestedOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedPenAnnotations();
        if (selectedAnnotations.length === 0) {
          message.warning("请先选中画笔对象");
          return false;
        }

        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as PenAnnotation);
        sourceBounds = resolvePenGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(requestedOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) =>
        offsetPenAnnotation(
          {
            ...annotation,
            id: crypto.randomUUID(),
          },
          offset,
        ),
      );

      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      clearTextSelection();
      clearShapeSelection();
      clearEffectSelection();
      clearNumberSelection();
      setPenSelection(
        duplicatedItems.map((annotation) => annotation.id),
        duplicatedItems[duplicatedItems.length - 1]?.id ?? null,
        nextAnnotations,
        duplicatedItems[duplicatedItems.length - 1] ?? null,
      );
      objectClipboardKindRef.current = "pen";

      if (mode === "duplicate") {
        if (duplicatedItems.length === 1) {
          message.success("已重复画笔路径");
        } else {
          message.success(`已重复 ${duplicatedItems.length} 条画笔路径`);
        }
      } else {
        if (duplicatedItems.length === 1) {
          message.success("已粘贴画笔路径");
        } else {
          message.success(`已粘贴 ${duplicatedItems.length} 条画笔路径`);
        }
      }
      return true;
    },
    [clearEffectSelection, clearNumberSelection, clearShapeSelection, clearTextSelection, commitAnnotations, getSelectedPenAnnotations, message, selection, setPenSelection],
  );

  const writeNumberClipboard = useCallback((annotationsToCopy: NumberAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      numberClipboardRef.current = null;
      if (objectClipboardKindRef.current === "number") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    numberClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as NumberAnnotation),
      groupBounds: resolveNumberGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "number";
  }, []);

  const copySelectedNumber = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedNumberAnnotations();
      if (selectedAnnotations.length === 0) {
        if (showFeedback) {
          message.warning("请先选中编号对象");
        }
        return false;
      }

      writeNumberClipboard(selectedAnnotations);
      if (showFeedback) {
        if (selectedAnnotations.length === 1) {
          message.success(`已复制编号 ${selectedAnnotations[0].value}`);
        } else {
          message.success(`已复制 ${selectedAnnotations.length} 个编号对象`);
        }
      }
      return true;
    },
    [getSelectedNumberAnnotations, message, writeNumberClipboard],
  );

  const pasteNumberClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: NumberAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let requestedOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = numberClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的编号对象");
          return false;
        }

        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as NumberAnnotation);
        sourceBounds = clipboard.groupBounds;
        requestedOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedNumberAnnotations();
        if (selectedAnnotations.length === 0) {
          message.warning("请先选中编号对象");
          return false;
        }

        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as NumberAnnotation);
        sourceBounds = resolveNumberGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(requestedOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) => ({
        ...annotation,
        id: crypto.randomUUID(),
        point: {
          x: annotation.point.x + offset.x,
          y: annotation.point.y + offset.y,
        },
      }));

      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      clearTextSelection();
      clearEffectSelection();
      setNumberSelection(
        duplicatedItems.map((annotation) => annotation.id),
        duplicatedItems[duplicatedItems.length - 1]?.id ?? null,
        nextAnnotations,
        duplicatedItems[duplicatedItems.length - 1] ?? null,
      );
      objectClipboardKindRef.current = "number";

      if (mode === "duplicate") {
        if (duplicatedItems.length === 1) {
          message.success(`已重复编号 ${duplicatedItems[0].value}`);
        } else {
          message.success(`已重复 ${duplicatedItems.length} 个编号对象`);
        }
      } else if (duplicatedItems.length === 1) {
        message.success(`已粘贴编号 ${duplicatedItems[0].value}`);
      } else {
        message.success(`已粘贴 ${duplicatedItems.length} 个编号对象`);
      }
      return true;
    },
    [clearEffectSelection, clearTextSelection, commitAnnotations, getSelectedNumberAnnotations, message, selection, setNumberSelection],
  );

  const writeEffectClipboard = useCallback((annotationsToCopy: EffectAnnotation[]) => {
    if (annotationsToCopy.length === 0) {
      effectClipboardRef.current = null;
      if (objectClipboardKindRef.current === "effect") {
        objectClipboardKindRef.current = null;
      }
      return;
    }

    effectClipboardRef.current = {
      items: annotationsToCopy.map((annotation) => cloneAnnotation(annotation) as EffectAnnotation),
      groupBounds: resolveEffectGroupBounds(annotationsToCopy),
      pasteCount: 0,
    };
    objectClipboardKindRef.current = "effect";
  }, []);

  const createDuplicatedObjectSelectionAnnotation = useCallback(
    (annotation: ObjectSelectionAnnotation, offset: Point): ObjectSelectionAnnotation =>
      offsetObjectSelectionAnnotation(
        {
          ...(cloneAnnotation(annotation) as ObjectSelectionAnnotation),
          id: crypto.randomUUID(),
        },
        offset,
      ),
    [],
  );

  const copySelectedEffect = useCallback(
    (showFeedback = true) => {
      const selectedAnnotations = getSelectedEffectAnnotations();
      if (selectedAnnotations.length === 0) {
        if (showFeedback) {
          message.warning("请先选中效果对象");
        }
        return false;
      }

      writeEffectClipboard(selectedAnnotations);
      if (showFeedback) {
        if (selectedAnnotations.length === 1) {
          message.success(`已复制${selectedAnnotations[0].effect === "mosaic" ? "马赛克" : "模糊"}区域`);
        } else {
          message.success(`已复制 ${selectedAnnotations.length} 个效果对象`);
        }
      }
      return true;
    },
    [getSelectedEffectAnnotations, message, writeEffectClipboard],
  );

  const pasteEffectClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: EffectAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let requestedOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = effectClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的效果对象");
          return false;
        }

        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as EffectAnnotation);
        sourceBounds = clipboard.groupBounds;
        requestedOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedEffectAnnotations();
        if (selectedAnnotations.length === 0) {
          message.warning("请先选中效果对象");
          return false;
        }

        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as EffectAnnotation);
        sourceBounds = resolveEffectGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(requestedOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) =>
        createEffectAnnotationWithBounds(
          {
            ...annotation,
            id: crypto.randomUUID(),
          },
          offsetRect(resolveEffectAnnotationBounds(annotation), offset),
        ),
      );

      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      clearTextSelection();
      clearNumberSelection();
      setEffectSelection(
        duplicatedItems.map((annotation) => annotation.id),
        duplicatedItems[duplicatedItems.length - 1]?.id ?? null,
        nextAnnotations,
        duplicatedItems[duplicatedItems.length - 1] ?? null,
      );
      objectClipboardKindRef.current = "effect";

      if (mode === "duplicate") {
        if (duplicatedItems.length === 1) {
          message.success(`已重复${duplicatedItems[0].effect === "mosaic" ? "马赛克" : "模糊"}区域`);
        } else {
          message.success(`已重复 ${duplicatedItems.length} 个效果对象`);
        }
      } else if (duplicatedItems.length === 1) {
        message.success(`已粘贴${duplicatedItems[0].effect === "mosaic" ? "马赛克" : "模糊"}区域`);
      } else {
        message.success(`已粘贴 ${duplicatedItems.length} 个效果对象`);
      }
      return true;
    },
    [clearNumberSelection, clearTextSelection, commitAnnotations, getSelectedEffectAnnotations, message, selection, setEffectSelection],
  );

  const pasteMixedClipboard = useCallback(
    (mode: "clipboard" | "duplicate") => {
      if (!selection) {
        message.warning("请先框选截图区域");
        return false;
      }

      let sourceItems: ObjectSelectionAnnotation[] = [];
      let sourceBounds: SelectionRect | null = null;
      let requestedOffset: Point = { x: 24, y: 24 };

      if (mode === "clipboard") {
        const clipboard = mixedClipboardRef.current;
        if (!clipboard || clipboard.items.length === 0) {
          message.warning("当前没有可粘贴的混选对象");
          return false;
        }

        clipboard.pasteCount += 1;
        sourceItems = clipboard.items.map((annotation) => cloneAnnotation(annotation) as ObjectSelectionAnnotation);
        sourceBounds = clipboard.groupBounds;
        requestedOffset = { x: clipboard.pasteCount * 24, y: clipboard.pasteCount * 24 };
      } else {
        const selectedAnnotations = getSelectedObjectAnnotations();
        const buckets = splitObjectAnnotationsToBuckets(selectedAnnotations);
        const selectedFamilyCountForDuplicate = [
          buckets.text.length > 0,
          buckets.shape.length > 0,
          buckets.pen.length > 0,
          buckets.number.length > 0,
          buckets.effect.length > 0,
        ].filter(Boolean).length;
        if (selectedAnnotations.length === 0 || selectedFamilyCountForDuplicate < 2) {
          message.warning("请先建立跨家族混选");
          return false;
        }

        sourceItems = selectedAnnotations.map((annotation) => cloneAnnotation(annotation) as ObjectSelectionAnnotation);
        sourceBounds = resolveObjectSelectionGroupBounds(selectedAnnotations);
      }

      if (!sourceBounds || sourceItems.length === 0) {
        return false;
      }

      const offset = resolvePasteOffset(requestedOffset, sourceBounds, selection);
      const duplicatedItems = sourceItems.map((annotation) => createDuplicatedObjectSelectionAnnotation(annotation, offset));
      const nextAnnotations = [...annotationsRef.current, ...duplicatedItems];
      commitAnnotations(nextAnnotations);
      restoreObjectSelections(splitObjectAnnotationsToBuckets(duplicatedItems), nextAnnotations);
      objectClipboardKindRef.current = "mixed";

      if (mode === "duplicate") {
        message.success(`已重复 ${duplicatedItems.length} 个混选对象`);
      } else {
        message.success(`已粘贴 ${duplicatedItems.length} 个混选对象`);
      }
      return true;
    },
    [
      commitAnnotations,
      createDuplicatedObjectSelectionAnnotation,
      getSelectedObjectAnnotations,
      message,
      restoreObjectSelections,
      selection,
      splitObjectAnnotationsToBuckets,
    ],
  );

  const resolvePreferredPasteObjectKind = useCallback((): ObjectClipboardKind | null => {
    if (hasMixedFamilySelection) {
      return "mixed";
    }

    if (getSelectedTextIds().length > 0) {
      return "text";
    }

    if (getSelectedShapeIds().length > 0) {
      return "shape";
    }

    if (getSelectedPenIds().length > 0) {
      return "pen";
    }

    if (getSelectedNumberIds().length > 0) {
      return "number";
    }

    if (getSelectedEffectIds().length > 0) {
      return "effect";
    }

    const preferred = objectClipboardKindRef.current;
    if (preferred === "mixed" && mixedClipboardRef.current?.items.length) {
      return "mixed";
    }
    if (preferred === "text" && textClipboardRef.current?.items.length) {
      return "text";
    }
    if (preferred === "shape" && shapeClipboardRef.current?.items.length) {
      return "shape";
    }
    if (preferred === "pen" && penClipboardRef.current?.items.length) {
      return "pen";
    }
    if (preferred === "number" && numberClipboardRef.current) {
      return "number";
    }
    if (preferred === "effect" && effectClipboardRef.current) {
      return "effect";
    }

    if (mixedClipboardRef.current?.items.length) {
      return "mixed";
    }
    if (textClipboardRef.current?.items.length) {
      return "text";
    }
    if (shapeClipboardRef.current?.items.length) {
      return "shape";
    }
    if (penClipboardRef.current?.items.length) {
      return "pen";
    }
    if (numberClipboardRef.current) {
      return "number";
    }
    if (effectClipboardRef.current) {
      return "effect";
    }

    return null;
  }, [getSelectedEffectIds, getSelectedNumberIds, getSelectedPenIds, getSelectedShapeIds, getSelectedTextIds, hasMixedFamilySelection]);

  const resetOverlayViewState = useCallback(() => {
    activeSessionIdRef.current = null;
    pendingSessionIdRef.current = null;
    firstPaintSessionIdRef.current = null;
    previewRenderableRef.current = null;
    frozenToolbarRectRef.current = null;
    previousNativeSelectionDragModeRef.current = null;
    nativeInteractionRuntimeInFlightRef.current = false;
    nativeInteractionRuntimeInFlightKeyRef.current = null;
    nativeInteractionRuntimeAppliedKeyRef.current = null;
    nativeInteractionRuntimeQueuedRequestRef.current = null;
    nativeSelectionRuntimeBlockedUntilRef.current = 0;
    setNativeSelectionRuntimeBlockedUntil(0);
    setPreviewSurfaceReady(false);
    setPreviewImageVersion((current) => current + 1);
    setSession(null);
    resetSessionInteractionState();
  }, [resetSessionInteractionState]);

  const loadSession = useCallback(async () => {
    if (!runtimeAvailable) return;

    const fetchStartedAt = performance.now();
    try {
      const value = await getScreenshotSession();
      const fetchMs = performance.now() - fetchStartedAt;
      const isSameSession = activeSessionIdRef.current === value.sessionId;
      activeSessionIdRef.current = value.sessionId;
      pendingSessionIdRef.current = value.sessionId;
      if (!isSameSession) {
        frozenToolbarRectRef.current = null;
        previousNativeSelectionDragModeRef.current = null;
        nativeInteractionRuntimeInFlightRef.current = false;
        nativeInteractionRuntimeInFlightKeyRef.current = null;
        nativeInteractionRuntimeAppliedKeyRef.current = null;
        nativeInteractionRuntimeQueuedRequestRef.current = null;
        nativeSelectionRuntimeBlockedUntilRef.current = 0;
        setNativeSelectionRuntimeBlockedUntil(0);
      }
      if (!sessionLoadingSeenAtRef.current.has(value.sessionId)) {
        sessionLoadingSeenAtRef.current.set(value.sessionId, performance.now());
      }
      if (!isSameSession) {
        firstPaintSessionIdRef.current = null;
      }
      emitPipelineInfo(
        `[screenshot][pipeline] load_session_done session_id=${value.sessionId} status=${value.imageStatus} fetch_ms=${fetchMs.toFixed(
          1,
        )} image_data_url_bytes=${value.imageDataUrl.length} preview_image_path=${value.previewImagePath ?? ""} preview_transport=${value.previewTransport} native_preview_active=${value.nativePreviewActive} preview_pixels=${value.previewPixelWidth}x${value.previewPixelHeight} monitors=${value.monitors.length}`,
      );
      setSession(value);
      if (!isSameSession) {
        resetSessionInteractionState();
      }
    } catch (error) {
      const summary = getErrorSummary(error);
      if (summary.code === "SCREENSHOT_SESSION_NOT_FOUND") {
        resetOverlayViewState();
        return;
      }
      message.error(summary.message || "读取截图会话失败");
    }
  }, [message, resetOverlayViewState, runtimeAvailable]);

  const buildNativeInteractionExclusionRects = useCallback((): NativeInteractionExclusionRect[] => {
    const stage = stageRef.current;
    if (!stage) {
      return [];
    }

    const rects: NativeInteractionExclusionRect[] = [];
    const stageBounds = stage.getBoundingClientRect();
    const collectRect = (element: HTMLElement | null) => {
      if (!element) {
        return;
      }
      const bounds = element.getBoundingClientRect();
      if (bounds.width <= 0 || bounds.height <= 0) {
        return;
      }
      rects.push({
        x: clamp(bounds.left - stageBounds.left, 0, stageBounds.width),
        y: clamp(bounds.top - stageBounds.top, 0, stageBounds.height),
        width: clamp(bounds.width, 0, stageBounds.width),
        height: clamp(bounds.height, 0, stageBounds.height),
      });
    };

    collectRect(toolbarRef.current);
    collectRect(textEditorRef.current);
    return rects;
  }, []);

  const flushNativeInteractionRuntimeUpdate = useCallback(
    (request: NativeInteractionRuntimeRequest) => {
      const requestKey = buildNativeInteractionRuntimeRequestKey(request);
      if (
        !nativeInteractionRuntimeInFlightRef.current &&
        nativeInteractionRuntimeAppliedKeyRef.current === requestKey
      ) {
        emitPipelineInfo(
          `[screenshot][native-interaction] runtime_update_skipped reason=dedup_applied session_id=${request.sessionId} visible=${request.visible} mode=${request.mode}`,
        );
        return;
      }

      if (nativeInteractionRuntimeInFlightRef.current) {
        const queuedRequest = nativeInteractionRuntimeQueuedRequestRef.current;
        const queuedKey = queuedRequest
          ? buildNativeInteractionRuntimeRequestKey(queuedRequest)
          : null;
        if (
          nativeInteractionRuntimeInFlightKeyRef.current === requestKey ||
          queuedKey === requestKey
        ) {
          emitPipelineInfo(
            `[screenshot][native-interaction] runtime_update_skipped reason=dedup_inflight session_id=${request.sessionId} visible=${request.visible} mode=${request.mode}`,
          );
          return;
        }

        nativeInteractionRuntimeQueuedRequestRef.current = request;
        emitPipelineInfo(
          `[screenshot][native-interaction] runtime_update_queued session_id=${request.sessionId} visible=${request.visible} mode=${request.mode} replaced=${queuedRequest ? "true" : "false"}`,
        );
        return;
      }

      nativeInteractionRuntimeInFlightRef.current = true;
      nativeInteractionRuntimeInFlightKeyRef.current = requestKey;
      const runtimeSeq = ++nativeInteractionRuntimeSeqRef.current;
      emitPipelineInfo(
        `[screenshot][native-interaction] runtime_update_sent seq=${runtimeSeq} session_id=${request.sessionId} visible=${request.visible} mode=${request.mode} exclusion_rects=${request.exclusionRects.length} selection=${request.selection ? `${request.selection.x.toFixed(1)},${request.selection.y.toFixed(1)},${request.selection.width.toFixed(1)},${request.selection.height.toFixed(1)}` : "none"} active_shape=${request.activeShape?.id ?? "none"} candidates=${request.shapeCandidates.length}`,
      );
      void updateNativeInteractionRuntime(
        request.sessionId,
        request.visible,
        request.exclusionRects,
        request.mode,
        request.selection,
        request.activeShape,
        request.shapeCandidates,
        request.color,
        request.strokeWidth,
      )
        .then((value) => {
          emitPipelineInfo(
            `[screenshot][native-interaction] runtime_update_completed seq=${runtimeSeq} session_id=${request.sessionId} lifecycle_state=${value.lifecycleState} has_active_session=${value.hasActiveSession} drag_mode=${value.dragMode ?? "none"} selection_revision=${value.selectionRevision}`,
          );
          if (activeSessionIdRef.current === request.sessionId) {
            nativeInteractionRuntimeAppliedKeyRef.current = requestKey;
            setNativeInteractionState((current) =>
              areNativeInteractionStatesEqual(current, value) ? current : value,
            );
          }
        })
        .catch((error) => {
          const summary = getErrorSummary(error);
          if (activeSessionIdRef.current === request.sessionId) {
            emitPipelineError(
              `[screenshot][native-interaction] runtime_update_failed seq=${runtimeSeq} session_id=${request.sessionId} pending_session_id=${pendingSessionIdRef.current ?? "none"} active_session_id=${activeSessionIdRef.current ?? "none"} mode=${request.mode}`,
              error,
            );
            console.warn("update native interaction runtime failed", summary);
          }
        })
        .finally(() => {
          if (nativeInteractionRuntimeInFlightKeyRef.current === requestKey) {
            nativeInteractionRuntimeInFlightKeyRef.current = null;
          }
          nativeInteractionRuntimeInFlightRef.current = false;
          if (activeSessionIdRef.current !== request.sessionId) {
            nativeInteractionRuntimeQueuedRequestRef.current = null;
            return;
          }
          const queuedRequest = nativeInteractionRuntimeQueuedRequestRef.current;
          nativeInteractionRuntimeQueuedRequestRef.current = null;
          if (!queuedRequest) {
            return;
          }
          const queuedKey = buildNativeInteractionRuntimeRequestKey(queuedRequest);
          if (queuedKey === nativeInteractionRuntimeAppliedKeyRef.current) {
            emitPipelineInfo(
              `[screenshot][native-interaction] runtime_update_skipped reason=dedup_queued session_id=${queuedRequest.sessionId} visible=${queuedRequest.visible} mode=${queuedRequest.mode}`,
            );
            return;
          }
          queueMicrotask(() => {
            flushNativeInteractionRuntimeUpdate(queuedRequest);
          });
        });
    },
    [],
  );

  useEffect(() => {
    if (!nativeSelectionRuntimeBlockedUntil) {
      return;
    }
    const remainingMs = nativeSelectionRuntimeBlockedUntil - performance.now();
    if (remainingMs <= 0) {
      nativeSelectionRuntimeBlockedUntilRef.current = 0;
      setNativeSelectionRuntimeBlockedUntil(0);
      return;
    }
    const timeoutId = window.setTimeout(() => {
      nativeSelectionRuntimeBlockedUntilRef.current = 0;
      setNativeSelectionRuntimeBlockedUntil(0);
    }, remainingMs);
    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [nativeSelectionRuntimeBlockedUntil]);

  useEffect(() => {
    const currentDragMode = nativeInteractionState?.dragMode ?? null;
    const previousDragMode = previousNativeSelectionDragModeRef.current;
    const wasSelectionDrag =
      previousDragMode === "creating" ||
      previousDragMode === "moving" ||
      previousDragMode === "resizing";
    const isSelectionDrag =
      currentDragMode === "creating" ||
      currentDragMode === "moving" ||
      currentDragMode === "resizing";
    if (!isSelectionDrag && wasSelectionDrag) {
      const blockedUntil = performance.now() + NATIVE_SELECTION_RUNTIME_STABILIZE_MS;
      nativeSelectionRuntimeBlockedUntilRef.current = blockedUntil;
      setNativeSelectionRuntimeBlockedUntil(blockedUntil);
    }
    previousNativeSelectionDragModeRef.current = currentDragMode;
  }, [nativeInteractionState?.dragMode]);

  useEffect(() => {
    if (!runtimeAvailable || !session) {
      return;
    }

    if (nativeInteractionState?.dragMode) {
      emitPipelineInfo(
        `[screenshot][native-interaction] runtime_update_skipped reason=drag_active session_id=${session.sessionId} drag_mode=${nativeInteractionState.dragMode} tool=${tool} mode=${nativeInteractionRuntimeMode}`,
      );
      return;
    }

    const runtimeBlockedUntil = nativeSelectionRuntimeBlockedUntilRef.current;
    const runtimeSelectionCoolingDown =
      nativeInteractionRuntimeMode === "selection" && runtimeBlockedUntil > performance.now();
    if (runtimeSelectionCoolingDown) {
      emitPipelineInfo(
        `[screenshot][native-interaction] runtime_update_skipped reason=selection_cooldown session_id=${session.sessionId} blocked_until=${runtimeBlockedUntil.toFixed(1)} tool=${tool} mode=${nativeInteractionRuntimeMode}`,
      );
      return;
    }

    const pendingSessionId = pendingSessionIdRef.current;
    const activeSessionId = activeSessionIdRef.current;
    if (
      (pendingSessionId && pendingSessionId !== session.sessionId) ||
      (activeSessionId && activeSessionId !== session.sessionId)
    ) {
      emitPipelineInfo(
        `[screenshot][native-interaction] runtime_update_skipped reason=session_gate session_id=${session.sessionId} pending_session_id=${pendingSessionId ?? "none"} active_session_id=${activeSessionId ?? "none"} tool=${tool} mode=${nativeInteractionRuntimeMode}`,
      );
      return;
    }

    const visible = nativeInteractionRuntimeVisible;
    const exclusionRects = visible ? buildNativeInteractionExclusionRects() : [];
    flushNativeInteractionRuntimeUpdate({
      sessionId: session.sessionId,
      visible,
      exclusionRects,
      mode: nativeInteractionRuntimeMode,
      selection: nativeInteractionRuntimeSelection,
      activeShape: nativeInteractionActiveShape,
      shapeCandidates: nativeInteractionShapeCandidates,
      color,
      strokeWidth,
    });
  }, [
    buildNativeInteractionExclusionRects,
    busyAction,
    color,
    nativeInteractionActiveShape,
    nativeInteractionShapeCandidates,
    nativeInteractionRuntimeMode,
    nativeInteractionRuntimeSelection,
    nativeInteractionRuntimeVisible,
    nativeInteractionState?.dragMode,
    session,
    strokeWidth,
    textEditor,
    dragCurrent,
    dragStart,
    draft,
    effectGroupDrag,
    effectTransform,
    mixedGroupDrag,
    numberDrag,
    numberGroupDrag,
    objectSelectionMarquee,
    penGroupDrag,
    penTransform,
    shapeGroupDrag,
    shapeTransform,
    textDrag,
    flushNativeInteractionRuntimeUpdate,
    nativeSelectionRuntimeBlockedUntil,
  ]);

  useEffect(() => {
    if (nativeSelectionInteractionStabilizing) {
      if (!frozenToolbarRectRef.current && activeRect) {
        frozenToolbarRectRef.current = activeRect;
      }
      return;
    }
    frozenToolbarRectRef.current = activeRect;
  }, [activeRect, nativeSelectionInteractionStabilizing]);

  useEffect(() => {
    if (!runtimeAvailable) {
      return;
    }

    let disposed = false;
    let unlistenState: (() => void) | null = null;
    let unlistenRectCommitted: (() => void) | null = null;
    let unlistenShapeUpdated: (() => void) | null = null;

    void (async () => {
      try {
        const [detachState, detachRectCommitted, detachShapeUpdated] = await Promise.all([
          listenToNativeInteractionStateUpdatedEvents((event) => {
          if (disposed) {
            return;
          }
          if (session && event.sessionId && event.sessionId !== session.sessionId) {
            emitPipelineInfo(
              `[screenshot][native-interaction] state_updated_dropped reason=session_mismatch event_session_id=${event.sessionId} current_session_id=${session.sessionId} drag_mode=${event.dragMode ?? "none"} lifecycle_state=${event.lifecycleState}`,
            );
            return;
          }
          emitPipelineInfo(
            `[screenshot][native-interaction] state_updated_applied session_id=${event.sessionId ?? "none"} drag_mode=${event.dragMode ?? "none"} lifecycle_state=${event.lifecycleState} interaction_mode=${event.interactionMode} selection_revision=${event.selectionRevision}`,
          );
          setNativeInteractionState((current) =>
            areNativeInteractionStatesEqual(current, event) ? current : event,
          );
          }),
          listenToNativeInteractionShapeAnnotationCommittedEvents((event) => {
            if (disposed) {
              return;
            }
            if (!session || event.sessionId !== session.sessionId) {
              return;
            }
            const annotation: ShapeAnnotation = {
              id: crypto.randomUUID(),
              kind: event.kind,
              color: event.color,
              strokeWidth: event.strokeWidth,
              start: event.start,
              end: event.end,
            };
            pushAnnotation(annotation);
          }),
          listenToNativeInteractionShapeAnnotationUpdatedEvents((event) => {
            if (disposed) {
              return;
            }
            if (!session || event.sessionId !== session.sessionId) {
              return;
            }
            const nextAnnotation: ShapeAnnotation = {
              id: event.id,
              kind: event.kind,
              color: event.color,
              strokeWidth: event.strokeWidth,
              start: event.start,
              end: event.end,
            };
            const nextAnnotations = annotationsRef.current.map((annotation) => {
              if (
                (annotation.kind !== "line" &&
                  annotation.kind !== "rect" &&
                  annotation.kind !== "ellipse" &&
                  annotation.kind !== "arrow") ||
                annotation.id !== event.id
              ) {
                return annotation;
              }
              return nextAnnotation;
            });
            commitAnnotations(nextAnnotations);
            selectShapeAnnotation(nextAnnotation);
          }),
        ]);
        if (disposed) {
          detachState();
          detachRectCommitted();
          detachShapeUpdated();
          return;
        }
        unlistenState = detachState;
        unlistenRectCommitted = detachRectCommitted;
        unlistenShapeUpdated = detachShapeUpdated;
      } catch (error) {
        console.warn("listen native interaction events failed", getErrorSummary(error));
      }
    })();

    return () => {
      disposed = true;
      if (unlistenState) {
        unlistenState();
      }
      if (unlistenRectCommitted) {
        unlistenRectCommitted();
      }
      if (unlistenShapeUpdated) {
        unlistenShapeUpdated();
      }
    };
  }, [commitAnnotations, pushAnnotation, runtimeAvailable, selectShapeAnnotation, session]);

  useEffect(() => {
    if (!nativeInteractionRuntimeVisible) {
      return;
    }
    setDragStart(null);
    setDragCurrent(null);
    setDraft(null);
    if (nativeInteractionState?.selection !== undefined) {
      setSelection((current) =>
        areSelectionRectsEqual(current, nativeInteractionState.selection ?? null)
          ? current
          : (nativeInteractionState.selection ?? null),
      );
    }
  }, [nativeInteractionRuntimeVisible, nativeInteractionState]);

  useEffect(() => {
    const previousTool = previousToolRef.current;
    previousToolRef.current = tool;
    if (
      tool === "select" &&
      previousTool !== "select" &&
      (previousTool === "line" ||
        previousTool === "rect" ||
        previousTool === "ellipse" ||
        previousTool === "arrow")
    ) {
      clearShapeSelection();
    }
  }, [clearShapeSelection, tool]);

  useEffect(() => {
    const nativeActiveShapeId =
      nativeInteractionState?.activeShapeDraft?.id ??
      nativeInteractionState?.activeShape?.id ??
      null;
    if (
      !session ||
      !nativeActiveShapeId ||
      !nativeShapeDragActive
    ) {
      return;
    }

    const annotation = findShapeAnnotationById(annotationsRef.current, nativeActiveShapeId);
    if (!annotation) {
      return;
    }

    const currentShapeIds =
      selectedShapeIds.length > 0 ? selectedShapeIds : selectedShapeId ? [selectedShapeId] : [];
    if (currentShapeIds.length === 1 && currentShapeIds[0] === annotation.id) {
      return;
    }

    clearTextSelection();
    clearPenSelection();
    clearEffectSelection();
    clearNumberSelection();
    setShapeSelection([annotation.id], annotation.id, annotationsRef.current, annotation);
  }, [
    annotations,
    clearEffectSelection,
    clearNumberSelection,
    clearPenSelection,
    clearTextSelection,
    nativeShapeDragActive,
    nativeInteractionState?.activeShape?.id,
    nativeInteractionState?.activeShapeDraft?.id,
    selectedShapeId,
    selectedShapeIds,
    session,
    setShapeSelection,
  ]);

  const buildSelectionInput = useCallback((): ScreenshotSelectionInput | null => {
    if (!selection) return null;
    return {
      x: Math.max(0, Math.round(selection.x)),
      y: Math.max(0, Math.round(selection.y)),
      width: Math.max(1, Math.round(selection.width)),
      height: Math.max(1, Math.round(selection.height)),
    };
  }, [selection]);

  const buildRenderedImageInput = useCallback(
    async (selectionInput: ScreenshotSelectionInput): Promise<ScreenshotRenderedImageInput | null> => {
      if (!session || !selection || annotationsRef.current.length === 0) {
        return null;
      }
      const render = await getScreenshotSelectionRender(session.sessionId, selectionInput);
      const dataUrl = await renderAnnotatedSelectionDataUrl(render, selection, annotationsRef.current);
      return { dataUrl };
    },
    [selection, session],
  );

  const handleCancel = useCallback(async () => {
    if (!runtimeAvailable || busyAction) return;
    setBusyAction("cancel");
    try {
      if (session) {
        await cancelScreenshotSession(session.sessionId);
      }
      resetOverlayViewState();
    } catch (error) {
      const summary = getErrorSummary(error);
      message.error(summary.message || "取消截图失败");
    } finally {
      setBusyAction(null);
    }
  }, [busyAction, message, resetOverlayViewState, runtimeAvailable, session]);

  const handleEscapeAction = useCallback(() => {
    if (textEditorStateRef.current) {
      cancelTextEditor();
      return true;
    }
    if (busyAction) {
      return false;
    }
    if (textDrag) {
      setTextDrag(null);
      return true;
    }
    if (mixedGroupDrag) {
      setMixedGroupDrag(null);
      return true;
    }
    if (objectSelectionMarquee) {
      setObjectSelectionMarquee(null);
      return true;
    }
    if (numberGroupDrag) {
      setNumberGroupDrag(null);
      return true;
    }
    if (numberDrag) {
      setNumberDrag(null);
      return true;
    }
    if (shapeGroupDrag) {
      setShapeGroupDrag(null);
      return true;
    }
    if (shapeTransform) {
      setShapeTransform(null);
      return true;
    }
    if (penGroupDrag) {
      setPenGroupDrag(null);
      return true;
    }
    if (penTransform) {
      setPenTransform(null);
      return true;
    }
    if (effectGroupDrag) {
      setEffectGroupDrag(null);
      return true;
    }
    if (effectTransform) {
      setEffectTransform(null);
      return true;
    }
    if (draft) {
      setDraft(null);
      return true;
    }
    if (tool !== "select") {
      setTool("select");
      return true;
    }
    if (annotations.length > 0) {
      resetAnnotations();
      return true;
    }
    void handleCancel();
    return true;
  }, [
    annotations.length,
    busyAction,
    cancelTextEditor,
    draft,
    effectGroupDrag,
    effectTransform,
    handleCancel,
    mixedGroupDrag,
    numberDrag,
    numberGroupDrag,
    objectSelectionMarquee,
    penGroupDrag,
    penTransform,
    resetAnnotations,
    shapeGroupDrag,
    shapeTransform,
    textDrag,
    tool,
  ]);

  const handleCopy = useCallback(async () => {
    if (!runtimeAvailable || busyAction || !session) return;
    if (!sessionImageReady) {
      message.warning("截图底图仍在准备中，请稍候");
      return;
    }
    const selectionInput = buildSelectionInput();
    if (!selectionInput) {
      message.warning("请先拖拽选择截图区域");
      return;
    }

    setBusyAction("copy");
    try {
      commitTextEditor();
      const renderedImage = await buildRenderedImageInput(selectionInput);
      await copyScreenshotSelection(session.sessionId, selectionInput, renderedImage);
      resetOverlayViewState();
    } catch (error) {
      const summary = getErrorSummary(error);
      message.error(summary.message || "复制截图失败");
    } finally {
      setBusyAction(null);
    }
  }, [buildRenderedImageInput, buildSelectionInput, busyAction, commitTextEditor, message, resetOverlayViewState, runtimeAvailable, session, sessionImageReady]);

  const handleSave = useCallback(async () => {
    if (!runtimeAvailable || busyAction || !session) return;
    if (!sessionImageReady) {
      message.warning("截图底图仍在准备中，请稍候");
      return;
    }
    const selectionInput = buildSelectionInput();
    if (!selectionInput) {
      message.warning("请先拖拽选择截图区域");
      return;
    }

    setBusyAction("save");
    try {
      const filePath = await save({
        title: "保存截图",
        defaultPath: `bexo-screenshot-${formatNowForFileName()}.png`,
        filters: [{ name: "PNG Image", extensions: ["png"] }],
      });
      if (typeof filePath !== "string" || !filePath.trim()) return;

      commitTextEditor();
      const renderedImage = await buildRenderedImageInput(selectionInput);
      await saveScreenshotSelection(session.sessionId, selectionInput, filePath, renderedImage);
      resetOverlayViewState();
    } catch (error) {
      const summary = getErrorSummary(error);
      message.error(summary.message || "保存截图失败");
    } finally {
      setBusyAction(null);
    }
  }, [buildRenderedImageInput, buildSelectionInput, busyAction, commitTextEditor, message, resetOverlayViewState, runtimeAvailable, session, sessionImageReady]);

  const getPointFromClient = useCallback((clientX: number, clientY: number): Point | null => {
    const stage = stageRef.current;
    if (!stage) return null;
    const bounds = stage.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return null;
    return {
      x: clamp(clientX - bounds.left, 0, bounds.width),
      y: clamp(clientY - bounds.top, 0, bounds.height),
    };
  }, []);

  const handleStageDoubleClick = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      if (nativeInteractionPointerOwned) return;
      if (event.button !== 0 || busyAction || !selection || !session || !sessionImageReady) return;

      const point = getPointFromClient(event.clientX, event.clientY);
      if (!point) return;

      const hitText = findTextAnnotationAtPoint(annotationsRef.current, point);
      if (!hitText) return;

      event.preventDefault();
      clearShapeSelection();
      clearPenSelection();
      clearEffectSelection();
      clearNumberSelection();
      setPenGroupDrag(null);
      openTextEditor(hitText.point, hitText);
    },
    [busyAction, clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, getPointFromClient, nativeInteractionPointerOwned, openTextEditor, selection, session, sessionImageReady],
  );

  const handleStagePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (nativeInteractionPointerOwned) return;
      if (event.button !== 0 || busyAction || !session || !sessionImageReady) return;

      const point = getPointFromClient(event.clientX, event.clientY);
      if (!point) return;
      pendingSelectionRedrawOriginRef.current = null;

      if (textEditorStateRef.current) {
        event.preventDefault();
        commitTextEditor();
      }

      const additiveSelection = event.ctrlKey || event.metaKey;
      const hitText = selection ? findTextAnnotationAtPoint(annotationsRef.current, point) : null;
      const hitShape = selection ? findShapeAnnotationAtPoint(annotationsRef.current, point) : null;
      const hitPen = selection ? findPenAnnotationAtPoint(annotationsRef.current, point) : null;
      const hitNumber = selection ? findNumberAnnotationAtPoint(annotationsRef.current, point) : null;
      const hitEffect = selection ? findEffectAnnotationAtPoint(annotationsRef.current, point) : null;
      const selectedShapeHandleMode =
        selection &&
        getSelectedShapeIds().length === 1 &&
        selectedShapeAnnotation &&
        !additiveSelection &&
        (tool === "select" || tool === selectedShapeAnnotation.kind)
          ? findShapeHandleAtPoint(selectedShapeAnnotation, point)
          : null;
      const selectedBuckets = getSelectedObjectBuckets();
      const mixedDragCandidateIds = new Set<string>();
      if (hitText && selectedBuckets.text.includes(hitText.id)) {
        mixedDragCandidateIds.add(hitText.id);
      }
      if (hitShape && selectedBuckets.shape.includes(hitShape.id)) {
        mixedDragCandidateIds.add(hitShape.id);
      }
      if (hitPen && selectedBuckets.pen.includes(hitPen.id)) {
        mixedDragCandidateIds.add(hitPen.id);
      }
      if (hitNumber && selectedBuckets.number.includes(hitNumber.id)) {
        mixedDragCandidateIds.add(hitNumber.id);
      }
      if (hitEffect && selectedBuckets.effect.includes(hitEffect.id)) {
        mixedDragCandidateIds.add(hitEffect.id);
      }
      const mixedGroupDragTarget =
        selection && hasMixedFamilySelection && tool === "select" && !additiveSelection && mixedDragCandidateIds.size > 0
          ? ([...annotationsRef.current]
              .reverse()
              .find(
                (annotation): annotation is ObjectSelectionAnnotation =>
                  annotation.kind !== "fill" && mixedDragCandidateIds.has(annotation.id),
              ) ?? null)
          : null;
      if (selection && mixedGroupDragTarget) {
        event.preventDefault();
        setObjectSelectionMarquee(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setTextDrag(null);
        event.currentTarget.setPointerCapture(event.pointerId);
        const dragAnnotations = getSelectedObjectAnnotations();
        setMixedGroupDrag({
          ids: dragAnnotations.map((annotation) => annotation.id),
          originAnnotations: Object.fromEntries(
            dragAnnotations.map((annotation) => [annotation.id, cloneAnnotation(annotation) as ObjectSelectionAnnotation]),
          ),
          startPointer: point,
          delta: { x: 0, y: 0 },
          groupBounds: resolveObjectSelectionGroupBounds(dragAnnotations),
          moved: false,
        });
        return;
      }
      if (selection && hitText && (tool === "text" || tool === "select")) {
        event.preventDefault();
        if (!additiveSelection) {
          clearShapeSelection();
          clearPenSelection();
          clearEffectSelection();
          clearNumberSelection();
        }
        setObjectSelectionMarquee(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setPenGroupDrag(null);
        setEffectGroupDrag(null);
        setNumberGroupDrag(null);
        setMixedGroupDrag(null);
        if (additiveSelection) {
          selectTextAnnotation(hitText, { toggle: true });
          setTextDrag(null);
          return;
        }

        const shouldPreserveGroup = selectedTextIds.includes(hitText.id);
        const dragIds = shouldPreserveGroup && selectedTextIds.length > 0 ? selectedTextIds : [hitText.id];
        const dragAnnotations = dragIds
          .map((id) => findTextAnnotationById(annotationsRef.current, id))
          .filter((annotation): annotation is TextAnnotation => annotation !== null);
        const groupBounds = resolveTextGroupBounds(dragAnnotations);

        event.currentTarget.setPointerCapture(event.pointerId);
        selectTextAnnotation(hitText, { preserveGroup: shouldPreserveGroup });
        setTextDrag({
          ids: dragIds,
          originPoints: Object.fromEntries(dragAnnotations.map((annotation) => [annotation.id, { ...annotation.point }])),
          startPointer: point,
          delta: { x: 0, y: 0 },
          groupBounds,
          guides: [],
          moved: false,
        });
        return;
      }

      if (selection && selectedShapeAnnotation && selectedShapeHandleMode) {
        event.preventDefault();
        clearTextSelection();
        clearPenSelection();
        clearEffectSelection();
        clearNumberSelection();
        setObjectSelectionMarquee(null);
        setShapeGroupDrag(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        event.currentTarget.setPointerCapture(event.pointerId);
        setShapeTransform({
          id: selectedShapeAnnotation.id,
          mode: selectedShapeHandleMode,
          startPointer: point,
          originAnnotation: selectedShapeAnnotation,
          previewAnnotation: selectedShapeAnnotation,
          moved: false,
        });
        return;
      }

      if (selection && hitShape && (tool === "select" || tool === hitShape.kind)) {
        event.preventDefault();
        if (!additiveSelection) {
          clearTextSelection();
          clearPenSelection();
          clearEffectSelection();
          clearNumberSelection();
        }
        setObjectSelectionMarquee(null);
        setShapeGroupDrag(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        if (additiveSelection) {
          selectShapeAnnotation(hitShape, { toggle: true });
          setShapeTransform(null);
          return;
        }
        const selectedIds = getSelectedShapeIds();
        const shouldPreserveGroup = selectedIds.includes(hitShape.id) && selectedIds.length > 1;
        if (shouldPreserveGroup) {
          const dragIds = selectedIds;
          const dragAnnotations = dragIds
            .map((id) => findShapeAnnotationById(annotationsRef.current, id))
            .filter((annotation): annotation is ShapeAnnotation => annotation !== null);
          const groupBounds = resolveShapeGroupBounds(dragAnnotations);

          event.currentTarget.setPointerCapture(event.pointerId);
          setShapeSelection(dragIds, hitShape.id, annotationsRef.current, hitShape);
          setShapeGroupDrag({
            ids: dragIds,
            originAnnotations: Object.fromEntries(dragAnnotations.map((annotation) => [annotation.id, cloneAnnotation(annotation) as ShapeAnnotation])),
            startPointer: point,
            delta: { x: 0, y: 0 },
            groupBounds,
            moved: false,
          });
          setShapeTransform(null);
          return;
        }
        event.currentTarget.setPointerCapture(event.pointerId);
        selectShapeAnnotation(hitShape);
        setShapeTransform({
          id: hitShape.id,
          mode: "move",
          startPointer: point,
          originAnnotation: hitShape,
          previewAnnotation: hitShape,
          moved: false,
        });
        return;
      }

      if (selection && hitPen && (tool === "select" || tool === "pen")) {
        event.preventDefault();
        if (!additiveSelection) {
          clearTextSelection();
          clearShapeSelection();
          clearEffectSelection();
          clearNumberSelection();
        }
        setObjectSelectionMarquee(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setEffectTransform(null);
        setPenGroupDrag(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        if (additiveSelection) {
          selectPenAnnotation(hitPen, { toggle: true });
          setPenTransform(null);
          return;
        }

        const selectedIds = getSelectedPenIds();
        const shouldPreserveGroup = selectedIds.includes(hitPen.id) && selectedIds.length > 1;
        if (shouldPreserveGroup) {
          const dragIds = selectedIds;
          const dragAnnotations = dragIds
            .map((id) => findPenAnnotationById(annotationsRef.current, id))
            .filter((annotation): annotation is PenAnnotation => annotation !== null);
          const groupBounds = resolvePenGroupBounds(dragAnnotations);

          event.currentTarget.setPointerCapture(event.pointerId);
          setPenSelection(dragIds, hitPen.id, annotationsRef.current, hitPen);
          setPenGroupDrag({
            ids: dragIds,
            originAnnotations: Object.fromEntries(dragAnnotations.map((annotation) => [annotation.id, cloneAnnotation(annotation) as PenAnnotation])),
            startPointer: point,
            delta: { x: 0, y: 0 },
            groupBounds,
            moved: false,
          });
          setPenTransform(null);
          return;
        }

        event.currentTarget.setPointerCapture(event.pointerId);
        selectPenAnnotation(hitPen);
        setPenTransform({
          id: hitPen.id,
          startPointer: point,
          originAnnotation: hitPen,
          previewAnnotation: hitPen,
          moved: false,
        });
        return;
      }

      if (selection && hitNumber && (tool === "number" || tool === "select")) {
        event.preventDefault();
        if (!additiveSelection) {
          clearTextSelection();
          clearShapeSelection();
          clearPenSelection();
          clearEffectSelection();
        }
        setObjectSelectionMarquee(null);
        setShapeGroupDrag(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        if (additiveSelection) {
          selectNumberAnnotation(hitNumber, { toggle: true });
          setNumberDrag(null);
          setNumberGroupDrag(null);
          return;
        }

        const selectedIds = getSelectedNumberIds();
        const shouldPreserveGroup = selectedIds.includes(hitNumber.id) && selectedIds.length > 1;
        if (shouldPreserveGroup) {
          const dragIds = selectedIds;
          const dragAnnotations = dragIds
            .map((id) => findNumberAnnotationById(annotationsRef.current, id))
            .filter((annotation): annotation is NumberAnnotation => annotation !== null);
          const groupBounds = resolveNumberGroupBounds(dragAnnotations);

          event.currentTarget.setPointerCapture(event.pointerId);
          setNumberSelection(dragIds, hitNumber.id, annotationsRef.current, hitNumber);
          setNumberGroupDrag({
            ids: dragIds,
            originPoints: Object.fromEntries(dragAnnotations.map((annotation) => [annotation.id, { ...annotation.point }])),
            startPointer: point,
            delta: { x: 0, y: 0 },
            groupBounds,
            moved: false,
          });
          setNumberDrag(null);
          return;
        }

        setNumberGroupDrag(null);
        selectNumberAnnotation(hitNumber);
        event.currentTarget.setPointerCapture(event.pointerId);
        setNumberDrag({
          id: hitNumber.id,
          startPointer: point,
          originAnnotation: hitNumber,
          previewAnnotation: hitNumber,
          moved: false,
        });
        return;
      }

      const selectedEffectHandleMode =
        selection &&
        getSelectedEffectIds().length === 1 &&
        selectedEffectAnnotation &&
        !additiveSelection &&
        (tool === "select" || tool === selectedEffectAnnotation.effect)
          ? findEffectHandleAtPoint(selectedEffectAnnotation, point)
          : null;
      if (selection && selectedEffectAnnotation && selectedEffectHandleMode) {
        event.preventDefault();
        clearTextSelection();
        clearShapeSelection();
        clearPenSelection();
        clearNumberSelection();
        setObjectSelectionMarquee(null);
        setShapeGroupDrag(null);
        setPenGroupDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        event.currentTarget.setPointerCapture(event.pointerId);
        const bounds = resolveEffectAnnotationBounds(selectedEffectAnnotation);
        setEffectTransform({
          id: selectedEffectAnnotation.id,
          mode: selectedEffectHandleMode,
          startPointer: point,
          originBounds: bounds,
          previewBounds: bounds,
          moved: false,
        });
        return;
      }

      if (selection && hitEffect && (tool === "select" || tool === hitEffect.effect)) {
        event.preventDefault();
        if (!additiveSelection) {
          clearTextSelection();
          clearShapeSelection();
          clearPenSelection();
          clearNumberSelection();
        }
        setObjectSelectionMarquee(null);
        setNumberGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        if (additiveSelection) {
          selectEffectAnnotation(hitEffect, { toggle: true });
          setEffectTransform(null);
          setEffectGroupDrag(null);
          return;
        }

        const selectedIds = getSelectedEffectIds();
        const shouldPreserveGroup = selectedIds.includes(hitEffect.id) && selectedIds.length > 1;
        if (shouldPreserveGroup) {
          const dragIds = selectedIds;
          const dragAnnotations = dragIds
            .map((id) => findEffectAnnotationById(annotationsRef.current, id))
            .filter((annotation): annotation is EffectAnnotation => annotation !== null);
          const groupBounds = resolveEffectGroupBounds(dragAnnotations);

          event.currentTarget.setPointerCapture(event.pointerId);
          setEffectSelection(dragIds, hitEffect.id, annotationsRef.current, hitEffect);
          setEffectGroupDrag({
            ids: dragIds,
            originBounds: Object.fromEntries(dragAnnotations.map((annotation) => [annotation.id, resolveEffectAnnotationBounds(annotation)])),
            startPointer: point,
            delta: { x: 0, y: 0 },
            groupBounds,
            moved: false,
          });
          setEffectTransform(null);
          return;
        }

        setEffectGroupDrag(null);
        selectEffectAnnotation(hitEffect);
        event.currentTarget.setPointerCapture(event.pointerId);
        const bounds = resolveEffectAnnotationBounds(hitEffect);
        setEffectTransform({
          id: hitEffect.id,
          mode: "move",
          startPointer: point,
          originBounds: bounds,
          previewBounds: bounds,
          moved: false,
        });
        return;
      }

      const canStartObjectMarquee =
        !!selection &&
        tool === "select" &&
        isPointInRect(point, selection) &&
        annotationsRef.current.some(
          (annotation) =>
            annotation.kind === "text" ||
            annotation.kind === "number" ||
            annotation.kind === "effect" ||
            annotation.kind === "pen" ||
            annotation.kind === "line" ||
            annotation.kind === "rect" ||
            annotation.kind === "ellipse" ||
            annotation.kind === "arrow",
        );
      if (canStartObjectMarquee) {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragStart(null);
        setDragCurrent(null);
        setDraft(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        setObjectSelectionMarquee({
          startPointer: point,
          currentPointer: point,
          additive: additiveSelection,
        });
        return;
      }

      if (!selection) {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        selectionRedrawStartedWithSelectionRef.current = false;
        setDragStart(point);
        setDragCurrent(point);
        setDraft(null);
        clearShapeSelection();
        clearPenSelection();
        setObjectSelectionMarquee(null);
        clearTextSelection();
        clearEffectSelection();
        clearNumberSelection();
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        return;
      }

      if (tool === "select" && !isPointInRect(point, selection)) {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        clearTextSelection();
        clearShapeSelection();
        clearPenSelection();
        clearEffectSelection();
        clearNumberSelection();
        setObjectSelectionMarquee(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        pendingSelectionRedrawOriginRef.current = point;
        return;
      }

      if (tool === "select") {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        selectionRedrawStartedWithSelectionRef.current = true;
        setDragStart(point);
        setDragCurrent(point);
        setDraft(null);
        clearShapeSelection();
        clearPenSelection();
        setObjectSelectionMarquee(null);
        clearTextSelection();
        clearEffectSelection();
        clearNumberSelection();
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        setTextDrag(null);
        return;
      }

      if (!isPointInRect(point, selection)) {
        clearTextSelection();
        clearShapeSelection();
        clearPenSelection();
        clearEffectSelection();
        clearNumberSelection();
        setObjectSelectionMarquee(null);
        setShapeTransform(null);
        setShapeGroupDrag(null);
        setPenTransform(null);
        setPenGroupDrag(null);
        setEffectTransform(null);
        setNumberDrag(null);
        setNumberGroupDrag(null);
        setEffectGroupDrag(null);
        setMixedGroupDrag(null);
        return;
      }

      event.preventDefault();
      clearTextSelection();
      clearShapeSelection();
      clearPenSelection();
      clearEffectSelection();
      clearNumberSelection();
      setObjectSelectionMarquee(null);
      setShapeTransform(null);
      setShapeGroupDrag(null);
      setPenTransform(null);
      setPenGroupDrag(null);
      setEffectTransform(null);
      setNumberDrag(null);
      setNumberGroupDrag(null);
      setEffectGroupDrag(null);
      setMixedGroupDrag(null);

      if (tool === "fill") {
        pushAnnotation({ id: crypto.randomUUID(), kind: "fill", color, opacity: fillOpacity / 100 });
        return;
      }

      if (tool === "text") {
      openTextEditor(point);
      return;
      }

      if (tool === "number") {
        const nextAnnotation = clampNumberAnnotationToSelection(
          {
            id: crypto.randomUUID(),
            kind: "number",
            value: getNextNumberValue(annotationsRef.current),
            color,
            size: fontSize,
            point,
          },
          selection,
        );
        pushAnnotation(nextAnnotation);
        return;
      }

      event.currentTarget.setPointerCapture(event.pointerId);

      if (tool === "mosaic" || tool === "blur") {
        setDraft({
          id: crypto.randomUUID(),
          kind: "effect",
          effect: tool,
          intensity: tool === "mosaic" ? mosaicSize : blurRadius,
          start: point,
          end: point,
        });
        return;
      }

      if (tool === "pen") {
        setDraft({ id: crypto.randomUUID(), kind: "pen", color, strokeWidth, points: [point] });
        return;
      }

      setDraft({
        id: crypto.randomUUID(),
        kind: tool,
        color,
        strokeWidth,
        start: point,
        end: point,
      });
    },
    [
      blurRadius,
      busyAction,
      color,
      commitTextEditor,
      clearPenSelection,
      clearShapeSelection,
      fillOpacity,
      getPointFromClient,
      getSelectedObjectAnnotations,
      getSelectedObjectBuckets,
      mosaicSize,
      openTextEditor,
      pushAnnotation,
      getSelectedEffectIds,
      getSelectedNumberIds,
      getSelectedShapeIds,
      hasMixedFamilySelection,
      selectedTextIds,
      setObjectSelectionMarquee,
      setShapeSelection,
      selectNumberAnnotation,
      selectEffectAnnotation,
      selectPenAnnotation,
      selectShapeAnnotation,
      selectTextAnnotation,
      clearEffectSelection,
      clearNumberSelection,
      clearTextSelection,
      setEffectSelection,
      setNumberSelection,
      selectedEffectAnnotation,
      selectedShapeAnnotation,
      selection,
      session,
      strokeWidth,
      tool,
      nativeInteractionPointerOwned,
    ],
  );

  const handleStagePointerMove = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (nativeInteractionPointerOwned) return;
      const point = getPointFromClient(event.clientX, event.clientY);
      if (!point) return;

      if (objectSelectionMarquee && selection) {
        setObjectSelectionMarquee((current) => {
          if (!current) return current;
          return {
            ...current,
            currentPointer: {
              x: clamp(point.x, selection.x, selection.x + selection.width),
              y: clamp(point.y, selection.y, selection.y + selection.height),
            },
          };
        });
        return;
      }

      if (mixedGroupDrag && selection) {
        setMixedGroupDrag((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
          );
          return {
            ...current,
            delta,
            moved: current.moved || Math.abs(delta.x) >= 1 || Math.abs(delta.y) >= 1,
          };
        });
        return;
      }

      if (numberGroupDrag && selection) {
        setNumberGroupDrag((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
          );
          return {
            ...current,
            delta,
            moved: current.moved || Math.abs(delta.x) >= 1 || Math.abs(delta.y) >= 1,
          };
        });
        return;
      }

      if (effectGroupDrag && selection) {
        setEffectGroupDrag((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
          );
          return {
            ...current,
            delta,
            moved: current.moved || Math.abs(delta.x) >= 1 || Math.abs(delta.y) >= 1,
          };
        });
        return;
      }

      if (numberDrag && selection) {
        setNumberDrag((current) => {
          if (!current) return current;
          const previewAnnotation = clampNumberAnnotationToSelection(
            {
              ...current.originAnnotation,
              point: {
                x: current.originAnnotation.point.x + (point.x - current.startPointer.x),
                y: current.originAnnotation.point.y + (point.y - current.startPointer.y),
              },
            },
            selection,
          );
          return {
            ...current,
            previewAnnotation,
            moved: current.moved || !arePointsEqual(current.originAnnotation.point, previewAnnotation.point),
          };
        });
        return;
      }

      if (shapeGroupDrag && selection) {
        setShapeGroupDrag((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
          );
          return {
            ...current,
            delta,
            moved: current.moved || Math.abs(delta.x) >= 1 || Math.abs(delta.y) >= 1,
          };
        });
        return;
      }

      if (shapeTransform && selection) {
        setShapeTransform((current) => {
          if (!current) return current;
          const previewAnnotation = resolveShapeTransformAnnotation(
            current.mode,
            current.originAnnotation,
            current.startPointer,
            point,
            selection,
          );
          return {
            ...current,
            previewAnnotation,
            moved: current.moved || !areShapeAnnotationsEqual(current.originAnnotation, previewAnnotation),
          };
        });
        return;
      }

      if (penTransform && selection) {
        setPenTransform((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            resolvePenAnnotationBounds(current.originAnnotation),
            selection,
          );
          const previewAnnotation = offsetPenAnnotation(current.originAnnotation, delta);
          return {
            ...current,
            previewAnnotation,
            moved: current.moved || !arePenAnnotationsEqual(current.originAnnotation, previewAnnotation),
          };
        });
        return;
      }

      if (penGroupDrag && selection) {
        setPenGroupDrag((current) => {
          if (!current) return current;
          const delta = clampGroupDeltaToSelection(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
          );
          return {
            ...current,
            delta,
            moved: current.moved || Math.abs(delta.x) >= 1 || Math.abs(delta.y) >= 1,
          };
        });
        return;
      }

      if (effectTransform && selection) {
        setEffectTransform((current) => {
          if (!current) return current;
          const previewBounds = resolveEffectTransformBounds(current.mode, current.originBounds, current.startPointer, point, selection);
          return {
            ...current,
            previewBounds,
            moved: current.moved || !areSelectionRectsEqual(current.originBounds, previewBounds),
          };
        });
        return;
      }

      if (textDrag && selection) {
        setTextDrag((current) => {
          if (!current) return current;
          const snapped = resolveSnappedTextDrag(
            {
              x: point.x - current.startPointer.x,
              y: point.y - current.startPointer.y,
            },
            current.groupBounds,
            selection,
            annotationsRef.current,
            current.ids,
          );

          return {
            ...current,
            delta: snapped.delta,
            guides: snapped.guides,
            moved: current.moved || Math.abs(snapped.delta.x) >= 1 || Math.abs(snapped.delta.y) >= 1,
          };
        });
        return;
      }

      const pendingSelectionRedrawOrigin = pendingSelectionRedrawOriginRef.current;
      if (pendingSelectionRedrawOrigin && tool === "select") {
        if (
          Math.abs(point.x - pendingSelectionRedrawOrigin.x) >= 2 ||
          Math.abs(point.y - pendingSelectionRedrawOrigin.y) >= 2
        ) {
          pendingSelectionRedrawOriginRef.current = null;
          selectionRedrawStartedWithSelectionRef.current = Boolean(selection);
          setDragStart(pendingSelectionRedrawOrigin);
          setDragCurrent(point);
        }
        return;
      }

      if (dragStart && tool === "select") {
        setDragCurrent(point);
        return;
      }

      if (!draft || !selection) return;
      const clamped = {
        x: clamp(point.x, selection.x, selection.x + selection.width),
        y: clamp(point.y, selection.y, selection.y + selection.height),
      };

      if (draft.kind === "pen") {
        setDraft((current) => {
          if (!current || current.kind !== "pen") return current;
          const last = current.points[current.points.length - 1];
          if (distance(last, clamped) < 2) return current;
          return { ...current, points: [...current.points, clamped] };
        });
        return;
      }

      setDraft((current) => {
        if (!current || current.kind === "pen") return current;
        return { ...current, end: clamped };
      });
    },
    [dragStart, draft, effectGroupDrag, effectTransform, getPointFromClient, mixedGroupDrag, nativeInteractionPointerOwned, numberDrag, numberGroupDrag, objectSelectionMarquee, penGroupDrag, penTransform, selection, setObjectSelectionMarquee, shapeGroupDrag, shapeTransform, textDrag, tool],
  );

  const handleStagePointerUp = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (nativeInteractionPointerOwned) return;
      const point = getPointFromClient(event.clientX, event.clientY);
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      if (pendingSelectionRedrawOriginRef.current && tool === "select") {
        pendingSelectionRedrawOriginRef.current = null;
        selectionRedrawStartedWithSelectionRef.current = false;
        return;
      }

      if (objectSelectionMarquee) {
        const currentMarquee = objectSelectionMarquee;
        setObjectSelectionMarquee(null);
        const endPoint = selection
          ? {
              x: clamp((point ?? currentMarquee.currentPointer).x, selection.x, selection.x + selection.width),
              y: clamp((point ?? currentMarquee.currentPointer).y, selection.y, selection.y + selection.height),
            }
          : point ?? currentMarquee.currentPointer;
        const marqueeRect = normalizeRect(currentMarquee.startPointer, endPoint);

        if (marqueeRect.width < 2 && marqueeRect.height < 2) {
          if (!currentMarquee.additive) {
            clearTextSelection();
            clearShapeSelection();
            clearPenSelection();
            clearEffectSelection();
            clearNumberSelection();
          }
          return;
        }

        const preferredFamily = resolvePreferredObjectMarqueeFamily(
          getSelectedTextIds(),
          getSelectedShapeIds(),
          getSelectedPenIds(),
          getSelectedNumberIds(),
          getSelectedEffectIds(),
        );
        const resolvedSelection = resolveObjectMarqueeSelection(annotationsRef.current, marqueeRect, preferredFamily);

        if (!resolvedSelection.family || resolvedSelection.ids.length === 0) {
          if (!currentMarquee.additive) {
            clearTextSelection();
            clearShapeSelection();
            clearPenSelection();
            clearEffectSelection();
            clearNumberSelection();
          }
          return;
        }

        if (resolvedSelection.family === "shape") {
          if (!currentMarquee.additive) {
            clearTextSelection();
            clearPenSelection();
            clearEffectSelection();
            clearNumberSelection();
          }
          const baseIds = currentMarquee.additive ? getSelectedShapeIds() : [];
          const mergedIds = Array.from(new Set([...baseIds, ...resolvedSelection.ids]));
          const primaryId =
            resolvedSelection.primaryId && mergedIds.includes(resolvedSelection.primaryId)
              ? resolvedSelection.primaryId
              : mergedIds[mergedIds.length - 1];
          setShapeSelection(
            mergedIds,
            primaryId,
            annotationsRef.current,
            primaryId ? findShapeAnnotationById(annotationsRef.current, primaryId) : null,
          );
          return;
        }

        if (resolvedSelection.family === "text") {
          if (!currentMarquee.additive) {
            clearShapeSelection();
            clearPenSelection();
            clearEffectSelection();
            clearNumberSelection();
          }
          const baseIds = currentMarquee.additive ? getSelectedTextIds() : [];
          const mergedIds = Array.from(new Set([...baseIds, ...resolvedSelection.ids]));
          const primaryId =
            resolvedSelection.primaryId && mergedIds.includes(resolvedSelection.primaryId)
              ? resolvedSelection.primaryId
              : mergedIds[mergedIds.length - 1];
          setTextSelection(
            mergedIds,
            primaryId,
            annotationsRef.current,
            primaryId ? findTextAnnotationById(annotationsRef.current, primaryId) : null,
          );
          return;
        }

        if (resolvedSelection.family === "pen") {
          if (!currentMarquee.additive) {
            clearTextSelection();
            clearShapeSelection();
            clearEffectSelection();
            clearNumberSelection();
          }
          const baseIds = currentMarquee.additive ? getSelectedPenIds() : [];
          const mergedIds = Array.from(new Set([...baseIds, ...resolvedSelection.ids]));
          const primaryId =
            resolvedSelection.primaryId && mergedIds.includes(resolvedSelection.primaryId)
              ? resolvedSelection.primaryId
              : mergedIds[mergedIds.length - 1];
          setPenSelection(
            mergedIds,
            primaryId,
            annotationsRef.current,
            primaryId ? findPenAnnotationById(annotationsRef.current, primaryId) : null,
          );
          return;
        }

        if (resolvedSelection.family === "number") {
          if (!currentMarquee.additive) {
            clearTextSelection();
            clearShapeSelection();
            clearPenSelection();
            clearEffectSelection();
          }
          const baseIds = currentMarquee.additive ? getSelectedNumberIds() : [];
          const mergedIds = Array.from(new Set([...baseIds, ...resolvedSelection.ids]));
          const primaryId =
            resolvedSelection.primaryId && mergedIds.includes(resolvedSelection.primaryId)
              ? resolvedSelection.primaryId
              : mergedIds[mergedIds.length - 1];
          setNumberSelection(
            mergedIds,
            primaryId,
            annotationsRef.current,
            primaryId ? findNumberAnnotationById(annotationsRef.current, primaryId) : null,
          );
          return;
        }

        if (!currentMarquee.additive) {
          clearTextSelection();
          clearShapeSelection();
          clearPenSelection();
          clearNumberSelection();
        }
        const baseIds = currentMarquee.additive ? getSelectedEffectIds() : [];
        const mergedIds = Array.from(new Set([...baseIds, ...resolvedSelection.ids]));
        const primaryId =
          resolvedSelection.primaryId && mergedIds.includes(resolvedSelection.primaryId)
            ? resolvedSelection.primaryId
            : mergedIds[mergedIds.length - 1];
        setEffectSelection(
          mergedIds,
          primaryId,
          annotationsRef.current,
          primaryId ? findEffectAnnotationById(annotationsRef.current, primaryId) : null,
        );
        return;
      }

      if (mixedGroupDrag) {
        const currentDrag = mixedGroupDrag;
        setMixedGroupDrag(null);
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            const originAnnotation = currentDrag.originAnnotations[annotation.id];
            if (!originAnnotation) {
              return annotation;
            }
            return offsetObjectSelectionAnnotation(originAnnotation, currentDrag.delta);
          });
          commitAnnotations(nextAnnotations);
          restoreObjectSelections(getSelectedObjectBuckets(), nextAnnotations);
          return;
        }

        restoreObjectSelections(getSelectedObjectBuckets());
        return;
      }

      if (numberGroupDrag) {
        const currentDrag = numberGroupDrag;
        setNumberGroupDrag(null);
        const primaryId =
          selectedNumberId && currentDrag.ids.includes(selectedNumberId) ? selectedNumberId : currentDrag.ids[currentDrag.ids.length - 1];
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const selectedSet = new Set(currentDrag.ids);
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "number" || !selectedSet.has(annotation.id)) {
              return annotation;
            }
            const originPoint = currentDrag.originPoints[annotation.id] ?? annotation.point;
            return {
              ...annotation,
              point: {
                x: originPoint.x + currentDrag.delta.x,
                y: originPoint.y + currentDrag.delta.y,
              },
            };
          });
          commitAnnotations(nextAnnotations);
          setNumberSelection(currentDrag.ids, primaryId, nextAnnotations);
          return;
        }

        setNumberSelection(currentDrag.ids, primaryId);
        return;
      }

      if (shapeGroupDrag) {
        const currentDrag = shapeGroupDrag;
        setShapeGroupDrag(null);
        const primaryId =
          selectedShapeId && currentDrag.ids.includes(selectedShapeId) ? selectedShapeId : currentDrag.ids[currentDrag.ids.length - 1];
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const selectedSet = new Set(currentDrag.ids);
          let primaryAnnotation: ShapeAnnotation | null = null;
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if ((annotation.kind !== "line" && annotation.kind !== "rect" && annotation.kind !== "ellipse" && annotation.kind !== "arrow") || !selectedSet.has(annotation.id)) {
              return annotation;
            }
            const originAnnotation = currentDrag.originAnnotations[annotation.id] ?? annotation;
            const nextAnnotation = offsetShapeAnnotation(originAnnotation, currentDrag.delta);
            if (annotation.id === primaryId) {
              primaryAnnotation = nextAnnotation;
            }
            return nextAnnotation;
          });
          commitAnnotations(nextAnnotations);
          setShapeSelection(currentDrag.ids, primaryId, nextAnnotations, primaryAnnotation);
          return;
        }

        setShapeSelection(
          currentDrag.ids,
          primaryId,
          annotationsRef.current,
          primaryId ? findShapeAnnotationById(annotationsRef.current, primaryId) : null,
        );
        return;
      }

      if (effectGroupDrag) {
        const currentDrag = effectGroupDrag;
        setEffectGroupDrag(null);
        const primaryId =
          selectedEffectId && currentDrag.ids.includes(selectedEffectId) ? selectedEffectId : currentDrag.ids[currentDrag.ids.length - 1];
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const selectedSet = new Set(currentDrag.ids);
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "effect" || !selectedSet.has(annotation.id)) {
              return annotation;
            }
            const originBounds = currentDrag.originBounds[annotation.id] ?? resolveEffectAnnotationBounds(annotation);
            return createEffectAnnotationWithBounds(annotation, offsetRect(originBounds, currentDrag.delta));
          });
          commitAnnotations(nextAnnotations);
          setEffectSelection(currentDrag.ids, primaryId, nextAnnotations);
          return;
        }

        setEffectSelection(currentDrag.ids, primaryId);
        return;
      }

      if (penGroupDrag) {
        const currentDrag = penGroupDrag;
        setPenGroupDrag(null);
        const primaryId =
          selectedPenId && currentDrag.ids.includes(selectedPenId) ? selectedPenId : currentDrag.ids[currentDrag.ids.length - 1];
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const selectedSet = new Set(currentDrag.ids);
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "pen" || !selectedSet.has(annotation.id)) {
              return annotation;
            }
            const originAnnotation = currentDrag.originAnnotations[annotation.id] ?? annotation;
            return offsetPenAnnotation(originAnnotation, currentDrag.delta);
          });
          commitAnnotations(nextAnnotations);
          setPenSelection(
            currentDrag.ids,
            primaryId,
            nextAnnotations,
            primaryId ? findPenAnnotationById(nextAnnotations, primaryId) : null,
          );
          return;
        }

        setPenSelection(
          currentDrag.ids,
          primaryId,
          annotationsRef.current,
          primaryId ? findPenAnnotationById(annotationsRef.current, primaryId) : null,
        );
        return;
      }

      if (penTransform) {
        const currentTransform = penTransform;
        setPenTransform(null);
        if (currentTransform.moved && !arePenAnnotationsEqual(currentTransform.originAnnotation, currentTransform.previewAnnotation)) {
          let updatedAnnotation: PenAnnotation | null = null;
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "pen" || annotation.id !== currentTransform.id) {
              return annotation;
            }
            updatedAnnotation = currentTransform.previewAnnotation;
            return updatedAnnotation;
          });
          commitAnnotations(nextAnnotations);
          selectPenAnnotation(updatedAnnotation);
          return;
        }

        selectPenAnnotation(findPenAnnotationById(annotationsRef.current, currentTransform.id));
        return;
      }

      if (numberDrag) {
        const currentDrag = numberDrag;
        setNumberDrag(null);
        if (currentDrag.moved && !arePointsEqual(currentDrag.originAnnotation.point, currentDrag.previewAnnotation.point)) {
          let updatedAnnotation: NumberAnnotation | null = null;
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "number" || annotation.id !== currentDrag.id) {
              return annotation;
            }
            updatedAnnotation = currentDrag.previewAnnotation;
            return updatedAnnotation;
          });
          commitAnnotations(nextAnnotations);
          selectNumberAnnotation(updatedAnnotation);
          return;
        }

        selectNumberAnnotation(findNumberAnnotationById(annotationsRef.current, currentDrag.id));
        return;
      }

      if (shapeTransform) {
        const currentTransform = shapeTransform;
        setShapeTransform(null);
        if (currentTransform.moved && !areShapeAnnotationsEqual(currentTransform.originAnnotation, currentTransform.previewAnnotation)) {
          let updatedAnnotation: ShapeAnnotation | null = null;
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if ((annotation.kind !== "line" && annotation.kind !== "rect" && annotation.kind !== "ellipse" && annotation.kind !== "arrow") || annotation.id !== currentTransform.id) {
              return annotation;
            }
            updatedAnnotation = currentTransform.previewAnnotation;
            return updatedAnnotation;
          });
          commitAnnotations(nextAnnotations);
          selectShapeAnnotation(updatedAnnotation);
          return;
        }

        selectShapeAnnotation(findShapeAnnotationById(annotationsRef.current, currentTransform.id));
        return;
      }

      if (effectTransform) {
        const currentTransform = effectTransform;
        setEffectTransform(null);
        if (currentTransform.moved && !areSelectionRectsEqual(currentTransform.originBounds, currentTransform.previewBounds)) {
          let updatedAnnotation: EffectAnnotation | null = null;
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "effect" || annotation.id !== currentTransform.id) {
              return annotation;
            }
            updatedAnnotation = createEffectAnnotationWithBounds(annotation, currentTransform.previewBounds);
            return updatedAnnotation;
          });
          commitAnnotations(nextAnnotations);
          selectEffectAnnotation(updatedAnnotation);
          return;
        }

        selectEffectAnnotation(findEffectAnnotationById(annotationsRef.current, currentTransform.id));
        return;
      }

      if (textDrag) {
        const currentDrag = textDrag;
        setTextDrag(null);
        if (currentDrag.moved && (Math.abs(currentDrag.delta.x) >= 1 || Math.abs(currentDrag.delta.y) >= 1)) {
          const selectedSet = new Set(currentDrag.ids);
          const nextAnnotations = annotationsRef.current.map((annotation) => {
            if (annotation.kind !== "text" || !selectedSet.has(annotation.id)) {
              return annotation;
            }
            const originPoint = currentDrag.originPoints[annotation.id] ?? annotation.point;
            return {
              ...annotation,
              point: {
                x: originPoint.x + currentDrag.delta.x,
                y: originPoint.y + currentDrag.delta.y,
              },
            };
          });
          commitAnnotations(nextAnnotations);
          const primaryId =
            activeTextId && currentDrag.ids.includes(activeTextId) ? activeTextId : currentDrag.ids[currentDrag.ids.length - 1];
          setTextSelection(currentDrag.ids, primaryId, nextAnnotations);
          return;
        }

        const primaryId =
          activeTextId && currentDrag.ids.includes(activeTextId) ? activeTextId : currentDrag.ids[currentDrag.ids.length - 1];
        setTextSelection(currentDrag.ids, primaryId);
        return;
      }

      if (dragStart && tool === "select") {
        const startedWithSelection = selectionRedrawStartedWithSelectionRef.current;
        selectionRedrawStartedWithSelectionRef.current = false;
        setDragStart(null);
        setDragCurrent(null);
        if (!point) return;
        const nextSelection = normalizeRect(dragStart, point);
        if (nextSelection.width < 2 || nextSelection.height < 2) {
          if (!startedWithSelection) {
            setSelection(null);
            resetAnnotations();
          }
          return;
        }
        setSelection(nextSelection);
        resetAnnotations();
        return;
      }

      if (!draft) return;
      if (draft.kind === "pen") {
        if (draft.points.length >= 2) {
          pushAnnotation(draft);
        }
      } else {
        const rect = normalizeRect(draft.start, draft.end);
        if (rect.width >= 2 || rect.height >= 2 || draft.kind === "line" || draft.kind === "arrow") {
          pushAnnotation(draft);
        }
      }
      setDraft(null);
    },
    [activeTextId, clearEffectSelection, clearNumberSelection, clearPenSelection, clearShapeSelection, clearTextSelection, commitAnnotations, dragStart, draft, effectGroupDrag, effectTransform, getPointFromClient, getSelectedEffectIds, getSelectedNumberIds, getSelectedObjectBuckets, getSelectedPenIds, getSelectedShapeIds, mixedGroupDrag, nativeInteractionPointerOwned, numberDrag, numberGroupDrag, objectSelectionMarquee, penGroupDrag, penTransform, pushAnnotation, resetAnnotations, restoreObjectSelections, selectEffectAnnotation, selectNumberAnnotation, selectPenAnnotation, selectShapeAnnotation, selectedEffectId, selectedNumberId, selectedPenId, selectedShapeId, selection, session, sessionImageReady, setEffectSelection, setNumberSelection, setObjectSelectionMarquee, setPenSelection, setShapeSelection, setTextSelection, shapeGroupDrag, shapeTransform, textDrag, tool],
  );

  useEffect(() => {
    void loadSession();
  }, [loadSession]);

  useEffect(() => {
    if (!runtimeAvailable) return;
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void (async () => {
      try {
        const detach = await listenToScreenshotSessionUpdatedEvents((event) => {
          const isNewSession = activeSessionIdRef.current !== event.sessionId;
          pendingSessionIdRef.current = event.sessionId;
          if (isNewSession) {
            previewRenderableRef.current = null;
            setPreviewSurfaceReady(false);
            setPreviewImageVersion((current) => current + 1);
            firstPaintSessionIdRef.current = null;
            sessionLoadingSeenAtRef.current.set(event.sessionId, performance.now());
            resetSessionInteractionState();
          }
          emitPipelineInfo(
            `[screenshot][pipeline] session_updated_event session_id=${event.sessionId} created_at=${event.createdAt} received_ms=${performance
              .now()
              .toFixed(1)}`,
          );
          void loadSession();
        });
        if (disposed) {
          detach();
          return;
        }
        unlisten = detach;
      } catch (error) {
        console.error("listen screenshot event failed", error);
      }
    })();

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [loadSession, resetSessionInteractionState, runtimeAvailable]);

  useEffect(() => {
    previewRenderableRef.current = null;
    setPreviewSurfaceReady(false);
    setPreviewImageVersion((current) => current + 1);
    firstPaintSessionIdRef.current = null;

    if (!session || session.imageStatus !== "ready") {
      return;
    }

    const sessionId = session.sessionId;
    let disposed = false;
    if (!previewImageSource) {
      return;
    }
    if (session.nativePreviewActive && !hasEffectPreview) {
      emitPipelineInfo(
        `[screenshot][pipeline] preview_image_decode_skipped session_id=${sessionId} reason=native_preview_active`,
      );
      return;
    }

    const imageSourceKind = previewImageSourceKind;
    const imageSourceSize =
      imageSourceKind === "data_url" ? session.imageDataUrl.length : previewImageSource.length;
    const decodeStartedAt = performance.now();
    emitPipelineInfo(
      `[screenshot][pipeline] preview_image_decode_start session_id=${sessionId} source_kind=${imageSourceKind} source_size=${imageSourceSize}`,
    );
    void loadImage(previewImageSource)
      .then((image) => {
        if (disposed) return;
        const decodeMs = performance.now() - decodeStartedAt;
        const loadingSeenAt = sessionLoadingSeenAtRef.current.get(sessionId);
        const fromLoadingSeenMs =
          typeof loadingSeenAt === "number" ? performance.now() - loadingSeenAt : Number.NaN;
        emitPipelineInfo(
          `[screenshot][pipeline] preview_image_decode_done session_id=${sessionId} decode_ms=${decodeMs.toFixed(
            1,
          )} from_loading_seen_ms=${
            Number.isFinite(fromLoadingSeenMs) ? fromLoadingSeenMs.toFixed(1) : "na"
          } source_kind=${imageSourceKind} source_size=${imageSourceSize} natural=${image.naturalWidth}x${image.naturalHeight}`,
        );
        previewRenderableRef.current = image;
        setPreviewSurfaceReady(true);
        if (!hasEffectPreview && !session.nativePreviewActive && firstPaintSessionIdRef.current !== sessionId) {
          firstPaintSessionIdRef.current = sessionId;
          emitPipelineInfo(
            `[screenshot][pipeline] preview_first_paint session_id=${sessionId} surface=img from_loading_seen_ms=${
              Number.isFinite(fromLoadingSeenMs) ? fromLoadingSeenMs.toFixed(1) : "na"
            } natural=${image.naturalWidth}x${image.naturalHeight}`,
          );
        }
        setPreviewImageVersion((current) => current + 1);
      })
      .catch((error) => {
        if (disposed) return;
        emitPipelineError("load preview image failed", error);
      });

    return () => {
      disposed = true;
    };
  }, [hasEffectPreview, previewImageSource, previewImageSourceKind, session?.imageStatus, session?.nativePreviewActive, session?.sessionId]);

  useEffect(() => {
    const canvas = previewCanvasRef.current;
    if (!canvas) return;
    if (!hasEffectPreview) {
      const context = canvas.getContext("2d");
      context?.clearRect(0, 0, canvas.width || 0, canvas.height || 0);
      return;
    }

    const drawStartedAt = performance.now();
    const context = canvas.getContext("2d");
    if (!context) return;

    const image = previewRenderableRef.current;
    if (!session || session.imageStatus !== "ready" || !image) {
      context.clearRect(0, 0, canvas.width || 0, canvas.height || 0);
      return;
    }

    canvas.width = Math.max(1, Math.round(session.displayWidth));
    canvas.height = Math.max(1, Math.round(session.displayHeight));
    drawEffectPreviewLayer(context, image, canvas.width, canvas.height, effectPreviewAnnotations);
    if (firstPaintSessionIdRef.current !== session.sessionId) {
      firstPaintSessionIdRef.current = session.sessionId;
      const loadingSeenAt = sessionLoadingSeenAtRef.current.get(session.sessionId);
      const fromLoadingSeenMs =
        typeof loadingSeenAt === "number" ? performance.now() - loadingSeenAt : Number.NaN;
      emitPipelineInfo(
        `[screenshot][pipeline] preview_canvas_first_paint session_id=${session.sessionId} draw_ms=${(
          performance.now() - drawStartedAt
        ).toFixed(1)} from_loading_seen_ms=${
          Number.isFinite(fromLoadingSeenMs) ? fromLoadingSeenMs.toFixed(1) : "na"
        } canvas=${canvas.width}x${canvas.height}`,
      );
    }
  }, [effectPreviewAnnotations, hasEffectPreview, previewImageVersion, session]);

  useEffect(() => {
    if (!textEditor) return;
    const frame = window.requestAnimationFrame(() => {
      const input = textEditorRef.current;
      if (!input) return;
      resizeTextEditor(input);
      input.focus();
      const length = input.value.length;
      input.setSelectionRange(length, length);
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [textEditor?.id, textEditor?.point.x, textEditor?.point.y]);

  useEffect(() => {
    if (tool === "text" || tool === "select") return;
    clearTextSelection();
  }, [clearTextSelection, tool]);

  useEffect(() => {
    if (tool === "select" || tool === "mosaic" || tool === "blur") return;
    clearEffectSelection();
  }, [clearEffectSelection, tool]);

  useEffect(() => {
    if (tool === "select" || tool === "number") return;
    clearNumberSelection();
  }, [clearNumberSelection, tool]);

  useEffect(() => {
    if (!activeTextId || textEditor || textDrag) return;
    const current = findTextAnnotationById(displayAnnotations, activeTextId);
    if (!current) {
      clearTextSelection();
      return;
    }
    if (current.style !== textStyle) {
      setTextStyle(current.style);
    }
    if (current.fontSize !== fontSize) {
      setFontSize(current.fontSize);
    }
    if (Math.round(current.rotation) !== textRotation) {
      setTextRotation(Math.round(current.rotation));
    }
    if (Math.round(current.opacity * 100) !== textOpacity) {
      setTextOpacity(Math.round(current.opacity * 100));
    }
  }, [activeTextId, clearTextSelection, displayAnnotations, fontSize, textDrag, textEditor, textOpacity, textRotation, textStyle]);

  useEffect(() => {
    if (!selectedEffectId || textEditor || textDrag || effectTransform || effectGroupDrag) return;
    const current = findEffectAnnotationById(displayAnnotations, selectedEffectId);
    if (!current) {
      clearEffectSelection();
      return;
    }
    syncEffectControls(current);
  }, [clearEffectSelection, displayAnnotations, effectGroupDrag, effectTransform, selectedEffectId, syncEffectControls, textDrag, textEditor]);

  useEffect(() => {
    if (!selectedNumberId || textEditor || textDrag || numberDrag || numberGroupDrag) return;
    const current = findNumberAnnotationById(displayAnnotations, selectedNumberId);
    if (!current) {
      clearNumberSelection();
      return;
    }
    if (current.size !== fontSize) {
      setFontSize(current.size);
    }
  }, [clearNumberSelection, displayAnnotations, fontSize, numberDrag, numberGroupDrag, selectedNumberId, textDrag, textEditor]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (textEditorStateRef.current) {
        if (event.key === "Escape") {
          if (handleEscapeAction()) {
            event.preventDefault();
          }
          return;
        }

        if (event.key === "Enter" && !event.shiftKey && !textCompositionRef.current) {
          event.preventDefault();
          commitTextEditor();
          return;
        }

        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
          event.preventDefault();
          void handleSave();
          return;
        }

        return;
      }

      if (event.key === "Escape") {
        if (handleEscapeAction()) {
          event.preventDefault();
        }
        return;
      }

      if (!busyAction && selection) {
        const selectedBuckets = getSelectedObjectBuckets();
        const selectedFamilyCountForKeys = [
          selectedBuckets.text.length > 0,
          selectedBuckets.shape.length > 0,
          selectedBuckets.pen.length > 0,
          selectedBuckets.number.length > 0,
          selectedBuckets.effect.length > 0,
        ].filter(Boolean).length;
        const hasMixedFamilySelectionForKeys = selectedFamilyCountForKeys > 1;

        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
          if (hasMixedFamilySelectionForKeys) {
            event.preventDefault();
            copySelectedMixedObjects();
            return;
          }

          if (selectedBuckets.text.length > 0) {
            event.preventDefault();
            copySelectedTexts();
            return;
          }

          if (selectedBuckets.shape.length > 0) {
            event.preventDefault();
            copySelectedShape();
            return;
          }

          if (selectedBuckets.pen.length > 0) {
            event.preventDefault();
            copySelectedPen();
            return;
          }

          if (selectedBuckets.number.length > 0) {
            event.preventDefault();
            copySelectedNumber();
            return;
          }

          if (selectedBuckets.effect.length > 0) {
            event.preventDefault();
            copySelectedEffect();
            return;
          }
        }

        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "v") {
          const targetKind = resolvePreferredPasteObjectKind();
          if (!targetKind) {
            event.preventDefault();
            message.warning("当前没有可粘贴的对象");
            return;
          }

          event.preventDefault();
          if (targetKind === "mixed") {
            pasteMixedClipboard("clipboard");
            return;
          }

          if (targetKind === "text") {
            pasteTextClipboard("clipboard");
            return;
          }

          if (targetKind === "shape") {
            pasteShapeClipboard("clipboard");
            return;
          }

          if (targetKind === "pen") {
            pastePenClipboard("clipboard");
            return;
          }

          if (targetKind === "number") {
            pasteNumberClipboard("clipboard");
            return;
          }

          pasteEffectClipboard("clipboard");
          return;
        }

        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "d") {
          if (hasMixedFamilySelectionForKeys) {
            event.preventDefault();
            pasteMixedClipboard("duplicate");
            return;
          }

          if (selectedBuckets.text.length > 0) {
            event.preventDefault();
            pasteTextClipboard("duplicate");
            return;
          }

          if (selectedBuckets.shape.length > 0) {
            event.preventDefault();
            pasteShapeClipboard("duplicate");
            return;
          }

          if (selectedBuckets.pen.length > 0) {
            event.preventDefault();
            pastePenClipboard("duplicate");
            return;
          }

          if (selectedBuckets.number.length > 0) {
            event.preventDefault();
            pasteNumberClipboard("duplicate");
            return;
          }

          if (selectedBuckets.effect.length > 0) {
            event.preventDefault();
            pasteEffectClipboard("duplicate");
            return;
          }
        }

        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
          if (selectAllObjects()) {
            event.preventDefault();
            return;
          }
        }

        const mappedTool = resolveToolHotkeyFromKeyboardEvent(event, toolHotkeyMap);
        if (mappedTool) {
          event.preventDefault();
          setTool(mappedTool);
          return;
        }

        if ((event.ctrlKey || event.metaKey) && event.code === "BracketLeft") {
          if (moveSelectedAnnotationLayer(event.shiftKey ? "back" : "backward")) {
            event.preventDefault();
            return;
          }
        }

        if ((event.ctrlKey || event.metaKey) && event.code === "BracketRight") {
          if (moveSelectedAnnotationLayer(event.shiftKey ? "front" : "forward")) {
            event.preventDefault();
            return;
          }
        }

        if (!event.shiftKey && event.code === "BracketLeft") {
          event.preventDefault();
          applyStrokeWidthValue(strokeWidth - 1);
          return;
        }

        if (!event.shiftKey && event.code === "BracketRight") {
          event.preventDefault();
          applyStrokeWidthValue(strokeWidth + 1);
          return;
        }
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") {
        event.preventDefault();
        undo();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "y" || (event.shiftKey && event.key.toLowerCase() === "z"))) {
        event.preventDefault();
        redo();
        return;
      }

      const selectedBuckets = getSelectedObjectBuckets();
      const hasSelectedText = selectedBuckets.text.length > 0;
      const hasSelectedShape = selectedBuckets.shape.length > 0;
      const hasSelectedPen = selectedBuckets.pen.length > 0;
      const hasSelectedNumber = selectedBuckets.number.length > 0;
      const hasSelectedEffect = selectedBuckets.effect.length > 0;
      const selectedFamilyCountForActions = [
        hasSelectedText,
        hasSelectedShape,
        hasSelectedPen,
        hasSelectedNumber,
        hasSelectedEffect,
      ].filter(Boolean).length;
      const hasMixedFamilySelectionForActions = selectedFamilyCountForActions > 1;

      if (hasMixedFamilySelectionForActions && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedMixedObjects();
        return;
      }

      if (hasSelectedText && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedTexts();
        return;
      }

      if (hasSelectedShape && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedShape();
        return;
      }

      if (hasSelectedPen && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedPen();
        return;
      }

      if (hasSelectedNumber && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedNumber();
        return;
      }

      if (hasSelectedEffect && (event.key === "Delete" || event.key === "Backspace")) {
        event.preventDefault();
        deleteSelectedEffect();
        return;
      }

      if (hasMixedFamilySelectionForActions && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedMixedObjects(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedMixedObjects(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedMixedObjects(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedMixedObjects(0, distance);
          return;
        }
        return;
      }

      if (hasSelectedText && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedTexts(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedTexts(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedTexts(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedTexts(0, distance);
          return;
        }
      }

      if (hasSelectedShape && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedShape(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedShape(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedShape(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedShape(0, distance);
          return;
        }
      }

      if (hasSelectedPen && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedPen(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedPen(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedPen(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedPen(0, distance);
          return;
        }
      }

      if (hasSelectedNumber && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedNumber(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedNumber(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedNumber(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedNumber(0, distance);
          return;
        }
      }

      if (hasSelectedEffect && selection && event.key.startsWith("Arrow")) {
        event.preventDefault();
        const distance = event.shiftKey ? 10 : 1;
        if (event.key === "ArrowLeft") {
          nudgeSelectedEffect(-distance, 0);
          return;
        }
        if (event.key === "ArrowRight") {
          nudgeSelectedEffect(distance, 0);
          return;
        }
        if (event.key === "ArrowUp") {
          nudgeSelectedEffect(0, -distance);
          return;
        }
        if (event.key === "ArrowDown") {
          nudgeSelectedEffect(0, distance);
          return;
        }
      }

      if (event.key === "Enter") {
        event.preventDefault();
        void handleCopy();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void handleSave();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [
    applyStrokeWidthValue,
    commitTextEditor,
      copySelectedEffect,
      copySelectedMixedObjects,
      copySelectedNumber,
      copySelectedPen,
      copySelectedShape,
    copySelectedTexts,
    deleteSelectedEffect,
    deleteSelectedMixedObjects,
    deleteSelectedNumber,
    deleteSelectedPen,
    deleteSelectedShape,
    deleteSelectedTexts,
    getSelectedObjectBuckets,
    handleEscapeAction,
    handleCopy,
    handleSave,
    message,
    moveSelectedAnnotationLayer,
    nudgeSelectedMixedObjects,
    nudgeSelectedEffect,
    nudgeSelectedNumber,
    nudgeSelectedPen,
    nudgeSelectedShape,
    nudgeSelectedTexts,
      pasteEffectClipboard,
      pasteMixedClipboard,
      pasteNumberClipboard,
      pastePenClipboard,
      pasteShapeClipboard,
    pasteTextClipboard,
    redo,
    resolvePreferredPasteObjectKind,
    selection,
    selectAllObjects,
    setObjectSelectionMarquee,
    strokeWidth,
    tool,
    toolHotkeyMap,
    undo,
  ]);

  useEffect(() => {
    if (!runtimeAvailable) {
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void (async () => {
      try {
        const detach = await listenToScreenshotEscapePressedEvents((event) => {
          if (disposed) {
            return;
          }
          if (activeSessionIdRef.current !== event.sessionId) {
            return;
          }
          handleEscapeAction();
        });
        if (disposed) {
          detach();
          return;
        }
        unlisten = detach;
      } catch (error) {
        console.warn("listen screenshot escape event failed", error);
      }
    })();

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [handleEscapeAction, runtimeAvailable]);

  const hasMeaningfulSelection = Boolean(selection && selection.width >= 12 && selection.height >= 12);
  const toolbarDisabled = !hasMeaningfulSelection || !!busyAction;
  const hasSelectedText = selectedTextIds.length > 0;
  const hasSelectedShape = selectedShapeAnnotations.length > 0;
  const hasSelectedPen = selectedPenAnnotations.length > 0;
  const hasSelectedNumber = selectedNumberAnnotations.length > 0;
  const hasSelectedEffect = selectedEffectAnnotations.length > 0;
  const layerControlDisabled = toolbarDisabled || !!textEditor || (!hasSelectedText && !hasSelectedShape && !hasSelectedPen && !hasSelectedNumber && !hasSelectedEffect);
  const showFloatingToolbar = hasMeaningfulSelection;
  const showSelectionStatusBar =
    Boolean(objectSelectionMarquee) ||
    hasMixedFamilySelection ||
    hasSelectedText ||
    hasSelectedShape ||
    hasSelectedPen ||
    hasSelectedNumber ||
    hasSelectedEffect;
  const showTextStyleControls = showFloatingToolbar && (tool === "text" || hasSelectedText);
  const showStrokeControls =
    showFloatingToolbar &&
    (tool === "line" ||
      tool === "arrow" ||
      tool === "rect" ||
      tool === "ellipse" ||
      tool === "pen" ||
      hasSelectedShape ||
      hasSelectedPen);
  const showTextControls = showFloatingToolbar && (tool === "text" || hasSelectedText);
  const showFillControls = showFloatingToolbar && (tool === "rect" || tool === "ellipse" || hasSelectedShape);
  const showEffectControls = showFloatingToolbar && (tool === "mosaic" || tool === "blur" || hasSelectedEffect);
  const showObjectActionControls =
    showFloatingToolbar &&
    (hasMixedFamilySelection || hasSelectedText || hasSelectedShape || hasSelectedPen || hasSelectedNumber || hasSelectedEffect);
  const showWebFloatingToolbar = showFloatingToolbar;
  const objectSelectionPreviewOtherHits =
    objectSelectionPreview && objectSelectionPreview.family
      ? (["text", "shape", "pen", "number", "effect"] as ObjectSelectionFamily[])
          .filter((family) => family !== objectSelectionPreview.family && objectSelectionPreview.counts[family] > 0)
          .map((family) => formatObjectFamilySelectionSummary(family, objectSelectionPreview.counts[family]))
      : [];

  const statusBarModel = useMemo<SelectionStatusBarModel>(() => {
    if (objectSelectionMarquee && objectSelectionPreview) {
      if (objectSelectionPreview.family) {
        return {
          tone: "preview",
          title: `${objectSelectionMarquee.additive ? "追加预览" : "框选预览"} · ${formatObjectFamilySelectionSummary(
            objectSelectionPreview.family,
            objectSelectionPreview.ids.length,
          )}`,
          subtitle:
            objectSelectionPreviewOtherHits.length > 0
              ? `其他命中 ${objectSelectionPreviewOtherHits.join(" / ")}，当前仍保持单家族选择`
              : objectSelectionMarquee.additive
                ? "松开后会追加到当前选择"
                : "松开后会替换当前选择",
          chips: ["单家族框选", objectSelectionMarquee.additive ? "增量追加" : "替换当前选择", "实时预览"],
        };
      }

      return {
        tone: "preview",
        title: "框选预览 · 未命中对象",
        subtitle: "当前未命中文字/图形/画笔/编号/效果对象",
        chips: ["单家族框选", "继续拖框"],
      };
    }

    if (hasMixedFamilySelection) {
      return {
        tone: "selection",
        title: `跨家族混选 · ${totalSelectedObjectCount} 个对象 / ${selectedFamilyCount} 个家族`,
        subtitle: "当前已开放整组拖动、方向键微调、复制/重复/粘贴、删除、层级和全选",
        chips: ["整组拖动", "方向键/Shift 微调", "Ctrl/Cmd+C/V/D", "Delete 删除", "Ctrl+[ / ] 层级", "Ctrl/Cmd+A 全选"],
      };
    }

    if (selectedTextAnnotations.length > 1) {
      return {
        tone: "selection",
        title: `文字批处理 · ${selectedTextAnnotations.length} 个对象`,
        subtitle: "当前为文字多选状态，可做整组拖动和批量层级处理",
        chips: ["整组拖动", "复制/重复", "旋转/透明度", "层级", "方向键"],
      };
    }

    if (selectedTextAnnotation) {
      return {
        tone: "selection",
        title: "文字对象 · 单选",
        subtitle: "可双击编辑、拖动并修改样式、字号、旋转和透明度",
        chips: ["双击编辑", "拖动", "样式/字号", "旋转/透明度", "层级"],
      };
    }

    if (selectedShapeAnnotations.length > 1) {
      return {
        tone: "selection",
        title: `图形批处理 · ${selectedShapeAnnotations.length} 个对象`,
        subtitle: "当前为图形多选状态，可统一调整线宽和层级",
        chips: ["整组拖动", "复制/重复", "线宽", "层级", "方向键"],
      };
    }

    if (selectedShapeAnnotation) {
      return {
        tone: "selection",
        title: `图形对象 · ${getShapeKindLabel(selectedShapeAnnotation.kind)}`,
        subtitle: "可拖动、句柄编辑，并修改线宽",
        chips: ["句柄编辑", "拖动", "复制/重复", "线宽", "层级"],
      };
    }

    if (selectedPenAnnotations.length > 1) {
      return {
        tone: "selection",
        title: `画笔批处理 · ${selectedPenAnnotations.length} 条路径`,
        subtitle: "当前为画笔多选状态，可整组拖动和批量层级处理",
        chips: ["整组拖动", "复制/重复", "层级", "方向键", "删除"],
      };
    }

    if (selectedPenAnnotation) {
      return {
        tone: "selection",
        title: "画笔对象 · 单选",
        subtitle: "可拖动路径，并修改线宽和层级",
        chips: ["拖动", "复制/重复", "线宽", "层级", "方向键"],
      };
    }

    if (selectedNumberAnnotations.length > 1) {
      return {
        tone: "selection",
        title: `编号批处理 · ${selectedNumberAnnotations.length} 个对象`,
        subtitle: "当前为编号多选状态，可整组拖动和批量层级处理",
        chips: ["整组拖动", "复制/重复", "层级", "方向键", "删除"],
      };
    }

    if (selectedNumberAnnotation) {
      return {
        tone: "selection",
        title: `编号对象 · #${selectedNumberAnnotation.value}`,
        subtitle: "可拖动，并修改字号和层级",
        chips: ["拖动", "复制/重复", "字号", "层级", "方向键"],
      };
    }

    if (selectedEffectAnnotations.length > 1) {
      return {
        tone: "selection",
        title: `效果批处理 · ${selectedEffectAnnotations.length} 个区域`,
        subtitle: "当前为效果多选状态，可整组拖动和批量层级处理",
        chips: ["整组拖动", "复制/重复", "层级", "方向键", "删除"],
      };
    }

    if (selectedEffectAnnotation) {
      return {
        tone: "selection",
        title: `效果对象 · ${selectedEffectAnnotation.effect === "mosaic" ? "马赛克" : "模糊"}`,
        subtitle: "可拖动、缩放并调整效果强度",
        chips: ["拖动", "缩放", "强度", "复制/重复", "层级"],
      };
    }

    return {
      tone: "idle",
      title: "默认样式状态",
      subtitle: "未选中对象时，当前字号、旋转等设置会作用于新建对象",
      chips: ["文字默认样式", "编号默认字号", "工具热键可切换"],
    };
  }, [
    hasMixedFamilySelection,
    objectSelectionMarquee,
    objectSelectionPreview,
    objectSelectionPreviewOtherHits,
    selectedFamilyCount,
    selectedEffectAnnotation,
    selectedEffectAnnotations,
    selectedNumberAnnotation,
    selectedNumberAnnotations,
    selectedPenAnnotation,
    selectedPenAnnotations,
    selectedShapeAnnotation,
    selectedShapeAnnotations,
    selectedTextAnnotation,
    selectedTextAnnotations,
    totalSelectedObjectCount,
  ]);

  useEffect(() => {
    if (!showWebFloatingToolbar) {
      return;
    }

    const node = toolbarRef.current;
    if (!node) {
      return;
    }

    const syncWidth = () => {
      const nextWidth = Math.ceil(node.getBoundingClientRect().width);
      setToolbarMeasuredWidth((current) => (Math.abs(current - nextWidth) < 1 ? current : nextWidth));
    };

    syncWidth();

    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      syncWidth();
    });
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  }, [showWebFloatingToolbar]);

  const toolbarToolActions: ToolbarIconAction[] = TOOLS.map((item) => ({
    key: `tool-${item.key}`,
    label: item.label,
    icon: getToolbarToolIcon(item.key),
    active: tool === item.key,
    disabled: toolbarDisabled,
    onClick: () => setTool(item.key),
  }));

  const toolbarTextStyleActions: ToolbarIconAction[] = showTextStyleControls
    ? TEXT_STYLE_OPTIONS.map((item) => ({
        key: `text-style-${item.key}`,
        label: `文字样式 · ${item.label}`,
        icon: getToolbarTextStyleIcon(item.key),
        active: textStyle === item.key,
        disabled: toolbarDisabled,
        onClick: () => applyTextStyle(item.key),
      }))
    : [];

  const toolbarValueActions: ToolbarIconAction[] = [];
  if (showStrokeControls) {
    toolbarValueActions.push(
      {
        key: "stroke-width-dec",
        label: `线宽 -1 · 当前 ${strokeWidth}`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || strokeWidth <= 1,
        onClick: () => applyStrokeWidthValue(strokeWidth - 1),
      },
      {
        key: "stroke-width-inc",
        label: `线宽 +1 · 当前 ${strokeWidth}`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || strokeWidth >= 18,
        onClick: () => applyStrokeWidthValue(strokeWidth + 1),
      },
    );
  }
  if (showTextControls) {
    toolbarValueActions.push(
      {
        key: "font-size-dec",
        label: `字号 -2 · 当前 ${fontSize}`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || fontSize <= 10,
        onClick: () => applyFontSize(fontSize - 2),
      },
      {
        key: "font-size-inc",
        label: `字号 +2 · 当前 ${fontSize}`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || fontSize >= 64,
        onClick: () => applyFontSize(fontSize + 2),
      },
      {
        key: "text-rotate-left",
        label: `文字左转 15° · 当前 ${textRotation}°`,
        icon: <RotateCcw size={15} />,
        disabled: toolbarDisabled || textRotation <= -180,
        onClick: () => applyTextRotation(textRotation - 15),
      },
      {
        key: "text-rotate-right",
        label: `文字右转 15° · 当前 ${textRotation}°`,
        icon: <RotateCw size={15} />,
        disabled: toolbarDisabled || textRotation >= 180,
        onClick: () => applyTextRotation(textRotation + 15),
      },
      {
        key: "text-opacity-dec",
        label: `文字透明度 -10% · 当前 ${textOpacity}%`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || textOpacity <= 10,
        onClick: () => applyTextOpacityValue(textOpacity - 10),
      },
      {
        key: "text-opacity-inc",
        label: `文字透明度 +10% · 当前 ${textOpacity}%`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || textOpacity >= 100,
        onClick: () => applyTextOpacityValue(textOpacity + 10),
      },
    );
  }
  if (showFillControls) {
    toolbarValueActions.push(
      {
        key: "fill-opacity-dec",
        label: `填充透明度 -10% · 当前 ${fillOpacity}%`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || fillOpacity <= 5,
        onClick: () => setFillOpacity(clampNumber(fillOpacity - 10, 5, 90)),
      },
      {
        key: "fill-opacity-inc",
        label: `填充透明度 +10% · 当前 ${fillOpacity}%`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || fillOpacity >= 90,
        onClick: () => setFillOpacity(clampNumber(fillOpacity + 10, 5, 90)),
      },
    );
  }
  if (showEffectControls) {
    toolbarValueActions.push(
      {
        key: "mosaic-size-dec",
        label: `马赛克强度 -2 · 当前 ${mosaicSize}`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || mosaicSize <= 4,
        onClick: () => applyEffectIntensity("mosaic", mosaicSize - 2),
      },
      {
        key: "mosaic-size-inc",
        label: `马赛克强度 +2 · 当前 ${mosaicSize}`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || mosaicSize >= 48,
        onClick: () => applyEffectIntensity("mosaic", mosaicSize + 2),
      },
      {
        key: "blur-radius-dec",
        label: `模糊半径 -1 · 当前 ${blurRadius}`,
        icon: <Minus size={15} />,
        disabled: toolbarDisabled || blurRadius <= 2,
        onClick: () => applyEffectIntensity("blur", blurRadius - 1),
      },
      {
        key: "blur-radius-inc",
        label: `模糊半径 +1 · 当前 ${blurRadius}`,
        icon: <Plus size={15} />,
        disabled: toolbarDisabled || blurRadius >= 24,
        onClick: () => applyEffectIntensity("blur", blurRadius + 1),
      },
    );
  }

  const toolbarObjectActions: ToolbarIconAction[] = [];
  if (showObjectActionControls) {
    toolbarObjectActions.push(
      {
        key: "layer-back",
        label: "置底",
        icon: <ChevronsLeft size={15} />,
        disabled: layerControlDisabled,
        onClick: () => moveSelectedAnnotationLayer("back"),
      },
      {
        key: "layer-backward",
        label: "后移一层",
        icon: <ChevronLeft size={15} />,
        disabled: layerControlDisabled,
        onClick: () => moveSelectedAnnotationLayer("backward"),
      },
      {
        key: "layer-forward",
        label: "前移一层",
        icon: <ChevronRight size={15} />,
        disabled: layerControlDisabled,
        onClick: () => moveSelectedAnnotationLayer("forward"),
      },
      {
        key: "layer-front",
        label: "置顶",
        icon: <ChevronsRight size={15} />,
        disabled: layerControlDisabled,
        onClick: () => moveSelectedAnnotationLayer("front"),
      },
    );

    if (hasMixedFamilySelection) {
      toolbarObjectActions.push(
        {
          key: "mixed-copy",
          label: "复制所选对象",
          icon: <Copy size={15} />,
          disabled: !hasMixedFamilySelection,
          onClick: () => copySelectedMixedObjects(),
        },
        {
          key: "mixed-paste",
          label: "粘贴混选对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pasteMixedClipboard("clipboard"),
        },
        {
          key: "mixed-duplicate",
          label: "重复所选对象",
          icon: <CopyPlus size={15} />,
          disabled: toolbarDisabled || !hasMixedFamilySelection,
          onClick: () => pasteMixedClipboard("duplicate"),
        },
        {
          key: "mixed-delete",
          label: "删除所选对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasMixedFamilySelection,
          onClick: deleteSelectedMixedObjects,
        },
      );
    } else if (hasSelectedText) {
      toolbarObjectActions.push(
        {
          key: "text-copy",
          label: "复制文字对象",
          icon: <Copy size={15} />,
          disabled: !hasSelectedText,
          onClick: () => copySelectedTexts(),
        },
        {
          key: "text-paste",
          label: "粘贴文字对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pasteTextClipboard("clipboard"),
        },
        {
          key: "text-duplicate",
          label: "重复文字对象",
          icon: <CopyPlus size={15} />,
          disabled: !hasSelectedText,
          onClick: () => pasteTextClipboard("duplicate"),
        },
        {
          key: "text-delete",
          label: "删除文字对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasSelectedText,
          onClick: deleteSelectedTexts,
        },
      );
    } else if (hasSelectedShape) {
      toolbarObjectActions.push(
        {
          key: "shape-copy",
          label: "复制图形对象",
          icon: <Copy size={15} />,
          disabled: !hasSelectedShape,
          onClick: () => copySelectedShape(),
        },
        {
          key: "shape-paste",
          label: "粘贴图形对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pasteShapeClipboard("clipboard"),
        },
        {
          key: "shape-duplicate",
          label: "重复图形对象",
          icon: <CopyPlus size={15} />,
          disabled: !hasSelectedShape,
          onClick: () => pasteShapeClipboard("duplicate"),
        },
        {
          key: "shape-delete",
          label: "删除图形对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasSelectedShape,
          onClick: deleteSelectedShape,
        },
      );
    } else if (hasSelectedPen) {
      toolbarObjectActions.push(
        {
          key: "pen-copy",
          label: "复制画笔对象",
          icon: <Copy size={15} />,
          disabled: !hasSelectedPen,
          onClick: () => copySelectedPen(),
        },
        {
          key: "pen-paste",
          label: "粘贴画笔对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pastePenClipboard("clipboard"),
        },
        {
          key: "pen-duplicate",
          label: "重复画笔对象",
          icon: <CopyPlus size={15} />,
          disabled: !hasSelectedPen,
          onClick: () => pastePenClipboard("duplicate"),
        },
        {
          key: "pen-delete",
          label: "删除画笔对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasSelectedPen,
          onClick: deleteSelectedPen,
        },
      );
    } else if (hasSelectedNumber) {
      toolbarObjectActions.push(
        {
          key: "number-copy",
          label: "复制编号对象",
          icon: <Copy size={15} />,
          disabled: !hasSelectedNumber,
          onClick: () => copySelectedNumber(),
        },
        {
          key: "number-paste",
          label: "粘贴编号对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pasteNumberClipboard("clipboard"),
        },
        {
          key: "number-duplicate",
          label: "重复编号对象",
          icon: <CopyPlus size={15} />,
          disabled: !hasSelectedNumber,
          onClick: () => pasteNumberClipboard("duplicate"),
        },
        {
          key: "number-delete",
          label: "删除编号对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasSelectedNumber,
          onClick: deleteSelectedNumber,
        },
      );
    } else if (hasSelectedEffect) {
      toolbarObjectActions.push(
        {
          key: "effect-copy",
          label: "复制效果对象",
          icon: <Copy size={15} />,
          disabled: !hasSelectedEffect,
          onClick: () => copySelectedEffect(),
        },
        {
          key: "effect-paste",
          label: "粘贴效果对象",
          icon: <ClipboardPaste size={15} />,
          disabled: toolbarDisabled,
          onClick: () => pasteEffectClipboard("clipboard"),
        },
        {
          key: "effect-duplicate",
          label: "重复效果对象",
          icon: <CopyPlus size={15} />,
          disabled: !hasSelectedEffect,
          onClick: () => pasteEffectClipboard("duplicate"),
        },
        {
          key: "effect-delete",
          label: "删除效果对象",
          icon: <Trash2 size={15} />,
          danger: true,
          disabled: !hasSelectedEffect,
          onClick: deleteSelectedEffect,
        },
      );
    }
  }

  const toolbarSessionActions: ToolbarIconAction[] = [
    {
      key: "undo",
      label: "撤销",
      icon: <Undo2 size={15} />,
      disabled: toolbarDisabled || historyStack.length === 0,
      onClick: undo,
    },
    {
      key: "redo",
      label: "重做",
      icon: <Redo2 size={15} />,
      disabled: toolbarDisabled || redoStack.length === 0,
      onClick: redo,
    },
    {
      key: "copy",
      label: "复制截图",
      icon: <Copy size={15} />,
      disabled: toolbarDisabled,
      loading: busyAction === "copy",
      onClick: () => {
        void handleCopy();
      },
    },
    {
      key: "save",
      label: "保存截图",
      icon: <Save size={15} />,
      disabled: toolbarDisabled,
      loading: busyAction === "save",
      onClick: () => {
        void handleSave();
      },
    },
    {
      key: "cancel",
      label: "取消截图",
      icon: <X size={15} />,
      danger: true,
      loading: busyAction === "cancel",
      onClick: () => {
        void handleCancel();
      },
    },
  ];

  const toolbarSections = [
    toolbarToolActions,
    toolbarTextStyleActions,
    toolbarValueActions,
    toolbarObjectActions,
    toolbarSessionActions,
  ].filter((section) => section.length > 0);

  const toolbarAnchorRect =
    nativeSelectionInteractionStabilizing && frozenToolbarRectRef.current
      ? frozenToolbarRectRef.current
      : activeRect;
  const toolbarDisplayWidth = session?.displayWidth ?? 480;
  const toolbarSafeHalfWidth = Math.min(
    Math.max(Math.ceil(toolbarMeasuredWidth / 2) + 12, 120),
    Math.max((toolbarDisplayWidth - 24) / 2, 120),
  );
  const toolbarLeft = toolbarAnchorRect
    ? clamp(
        toolbarAnchorRect.x + toolbarAnchorRect.width / 2,
        toolbarSafeHalfWidth,
        Math.max(toolbarSafeHalfWidth, toolbarDisplayWidth - toolbarSafeHalfWidth),
      )
    : toolbarDisplayWidth / 2;
  const toolbarTop = toolbarAnchorRect
    ? clamp(toolbarAnchorRect.y + toolbarAnchorRect.height + 16, 24, (session?.displayHeight ?? 180) - 64)
    : 24;
  const selectionStatusTop = Math.max(12, toolbarTop - 76);
  const textEditorLayout = selection && textEditor ? resolveTextEditorLayout(textEditor, selection) : null;
  const textEditorVisual = textEditor ? resolveTextEditorVisual(textEditor) : null;

  if (!runtimeAvailable) {
    return (
      <div className="flex h-screen items-center justify-center bg-transparent text-white">
        <Typography.Text className="text-white">截图模式仅支持桌面端运行。</Typography.Text>
      </div>
    );
  }

  return (
    <div
      className="relative h-screen w-screen overflow-hidden bg-transparent text-white"
      ref={stageRef}
      onDoubleClick={handleStageDoubleClick}
      onPointerDown={handleStagePointerDown}
      onPointerMove={handleStagePointerMove}
      onPointerUp={handleStagePointerUp}
    >
      {session ? (
        <>
          {session.imageStatus === "ready" ? (
            <>
              {renderWebviewPreviewSurface && previewSurfaceReady && previewImageSource ? (
                <img
                  key={session.sessionId}
                  alt="screenshot"
                  className="pointer-events-none absolute inset-0 h-full w-full select-none object-fill"
                  draggable={false}
                  src={previewImageSource}
                />
              ) : null}
              {hasEffectPreview ? <canvas ref={previewCanvasRef} className="pointer-events-none absolute inset-0 h-full w-full" /> : null}
            </>
          ) : null}
          {nativeInteractionVisualsActive ? null : renderMask(activeRect)}
          {nativeInteractionVisualsActive ? null : activeRect ? <SelectionBorder rect={activeRect} /> : null}
          {objectSelectionRect ? (
            <ObjectSelectionMarqueeOverlay
              additive={objectSelectionMarquee?.additive ?? false}
              annotations={objectSelectionPreviewAnnotations}
              preview={objectSelectionPreview}
              rect={objectSelectionRect}
            />
          ) : null}
          {selection ? (
            <svg className="pointer-events-none absolute inset-0 h-full w-full">
              {displayAnnotations.map((annotation) => renderAnnotationSvg(annotation, selection))}
              {draft ? renderAnnotationSvg(draft, selection) : null}
              {textDrag?.guides.map((guide, index) => renderSnapGuide(guide, index))}
            </svg>
          ) : null}
          {selection && !textEditor && activeSelectionGroupOverlay ? <SelectionGroupOverlay items={activeSelectionGroupOverlay.items} rect={activeSelectionGroupOverlay.rect} /> : null}
          {selection && !textEditor
            ? selectedShapeAnnotations.map((annotation) => (
                <ShapeSelectionOverlay
                  key={annotation.id}
                  annotation={annotation}
                  primary={annotation.id === selectedShapeId}
                  selectedCount={hasMixedFamilySelection ? Math.max(2, selectedShapeAnnotations.length) : selectedShapeAnnotations.length}
                  showHint={annotation.id === selectedShapeId && selectedShapeAnnotations.length === 1 && !hasMixedFamilySelection}
                />
              ))
            : null}
          {selection && !textEditor
            ? selectedPenAnnotations.map((annotation) => (
                <PenSelectionOverlay
                  key={annotation.id}
                  annotation={annotation}
                  primary={annotation.id === selectedPenId}
                  showHint={annotation.id === selectedPenId && selectedPenAnnotations.length === 1 && !hasMixedFamilySelection}
                />
              ))
            : null}
          {selection && !textEditor
            ? selectedTextAnnotations.map((annotation) => (
                <TextSelectionOverlay
                  key={annotation.id}
                  annotation={annotation}
                  primary={annotation.id === activeTextId}
                  showHint={annotation.id === activeTextId && selectedTextAnnotations.length === 1 && !hasMixedFamilySelection}
                />
              ))
            : null}
          {selection && !textEditor
            ? selectedNumberAnnotations.map((annotation) => (
                <NumberSelectionOverlay
                  key={annotation.id}
                  annotation={annotation}
                  primary={annotation.id === selectedNumberId}
                  showHint={annotation.id === selectedNumberId && selectedNumberAnnotations.length === 1 && !hasMixedFamilySelection}
                />
              ))
            : null}
          {selection && !textEditor
            ? selectedEffectAnnotations.map((annotation) => (
                <EffectSelectionOverlay
                  key={annotation.id}
                  annotation={annotation}
                  primary={annotation.id === selectedEffectId}
                  showHandles={annotation.id === selectedEffectId && selectedEffectAnnotations.length === 1 && !hasMixedFamilySelection}
                  showHint={annotation.id === selectedEffectId && selectedEffectAnnotations.length === 1 && !hasMixedFamilySelection}
                />
              ))
            : null}
          {selection && textEditor && textEditorLayout ? (
            <div
              className="absolute z-30 rounded border border-white/25 bg-black/40 p-1 shadow-[0_10px_24px_rgba(0,0,0,0.28)] backdrop-blur-sm"
              style={{
                left: `${textEditorLayout.left}px`,
                top: `${textEditorLayout.top}px`,
                width: `${textEditorLayout.width}px`,
                minHeight: `${textEditorLayout.height}px`,
                backgroundColor: textEditorVisual?.containerBackground,
                borderColor: textEditorVisual?.containerBorder,
                padding: `${textEditorLayout.paddingY}px ${textEditorLayout.paddingX}px`,
                opacity: textEditor.opacity,
                transform: `rotate(${textEditor.rotation}deg)`,
                transformOrigin: "center center",
              }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              <textarea
                ref={textEditorRef}
                className="block w-full resize-none overflow-hidden border-none bg-transparent px-1 py-0.5 font-semibold text-white outline-none placeholder:text-white/35"
                placeholder="输入文字"
                rows={1}
                style={{
                  color: textEditorVisual?.textColor ?? textEditor.color,
                  fontSize: `${textEditor.fontSize}px`,
                  lineHeight: `${textEditorLayout.lineHeight}px`,
                  minHeight: `${Math.max(textEditorLayout.lineHeight, textEditorLayout.height - textEditorLayout.paddingY * 2)}px`,
                  caretColor: textEditorVisual?.caretColor ?? textEditor.color,
                  textShadow: textEditorVisual?.textShadow,
                }}
                value={textEditor.text}
                wrap="off"
                onBlur={(event) => {
                  const nextTarget = event.relatedTarget;
                  if (nextTarget instanceof Node && toolbarRef.current?.contains(nextTarget)) {
                    return;
                  }
                  commitTextEditor();
                }}
                onChange={(event) => {
                  const value = event.target.value;
                  updateTextEditor((current) => ({ ...current, text: value }));
                }}
                onCompositionEnd={() => {
                  textCompositionRef.current = false;
                }}
                onCompositionStart={() => {
                  textCompositionRef.current = true;
                }}
                onInput={(event) => {
                  resizeTextEditor(event.currentTarget);
                }}
              />
            </div>
          ) : null}
          {showSelectionStatusBar && showWebFloatingToolbar ? (
            <div
              className="absolute z-40 max-w-[calc(100vw-24px)] -translate-x-1/2"
              style={{ left: `${toolbarLeft}px`, top: `${selectionStatusTop}px` }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              <SelectionStatusBar model={statusBarModel} />
            </div>
          ) : null}
          {showWebFloatingToolbar ? (
            <div
              ref={toolbarRef}
              role="toolbar"
              aria-label="截图工具栏"
              className="absolute z-40 flex max-w-[calc(100vw-24px)] -translate-x-1/2 items-center gap-2 overflow-x-auto rounded-xl border border-white/14 bg-black/80 px-2 py-2 shadow-[0_16px_40px_rgba(0,0,0,0.35)] backdrop-blur"
              style={{ left: `${toolbarLeft}px`, top: `${toolbarTop}px` }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              {toolbarSections.map((section, index) => (
                <div key={`toolbar-section-${index}`} className="flex shrink-0 items-center gap-1">
                  {section.map((action) => (
                    <ToolbarIconButton key={action.key} action={action} />
                  ))}
                  {index < toolbarSections.length - 1 ? <ToolbarSectionDivider /> : null}
                </div>
              ))}
            </div>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function getToolbarToolIcon(tool: ToolKind): ReactNode {
  switch (tool) {
    case "select":
      return <MousePointer2 size={15} />;
    case "line":
      return <Minus size={15} />;
    case "rect":
      return <Square size={15} />;
    case "ellipse":
      return <Circle size={15} />;
    case "arrow":
      return <ArrowRight size={15} />;
    case "pen":
      return <Pencil size={15} />;
    case "text":
      return <Type size={15} />;
    case "number":
      return <Hash size={15} />;
    case "fill":
      return <PaintBucket size={15} />;
    case "mosaic":
      return <Grid2x2 size={15} />;
    case "blur":
      return <Droplets size={15} />;
    default:
      return <Square size={15} />;
  }
}

function getToolbarTextStyleIcon(style: TextStyleKind): ReactNode {
  switch (style) {
    case "plain":
      return <Type size={15} />;
    case "outline":
      return <SquareDashed size={15} />;
    case "background":
      return <Square size={15} />;
    case "highlight":
      return <Highlighter size={15} />;
    default:
      return <Type size={15} />;
  }
}

function ToolbarIconButton({ action }: { action: ToolbarIconAction }) {
  return (
    <Tooltip placement="top" title={action.label}>
      <span className="inline-flex shrink-0">
        <Button
          aria-label={action.label}
          className="!h-8 !w-8 !min-w-8 !shrink-0 !px-0"
          danger={action.danger}
          disabled={action.disabled}
          icon={action.icon}
          loading={action.loading}
          size="small"
          type={action.active ? "primary" : "default"}
          onClick={action.onClick}
        />
      </span>
    </Tooltip>
  );
}

function ToolbarSectionDivider() {
  return <div className="mx-1 h-5 w-px shrink-0 bg-white/12" aria-hidden="true" />;
}

function SelectionBorder({ rect }: { rect: SelectionRect }) {
  return (
    <div className="pointer-events-none absolute border-2 border-[#00d08f]" style={{ left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.width}px`, height: `${rect.height}px` }}>
      <div className="absolute -top-7 left-0 rounded bg-black/65 px-2 py-0.5 text-[11px] text-[#d7ffe9]">{Math.round(rect.width)} x {Math.round(rect.height)}</div>
    </div>
  );
}

function ObjectSelectionMarqueeOverlay({
  rect,
  preview,
  annotations,
  additive,
}: {
  rect: SelectionRect;
  preview: ObjectMarqueeResolution | null;
  annotations: ObjectSelectionAnnotation[];
  additive: boolean;
}) {
  const accentColor = !preview?.family ? "rgba(255, 255, 255, 0.9)" : additive ? "#2f95ff" : "#00d08f";
  const fillColor = !preview?.family ? "rgba(255, 255, 255, 0.06)" : additive ? "rgba(47, 149, 255, 0.10)" : "rgba(0, 208, 143, 0.08)";
  const mainLabel =
    preview?.family && preview.ids.length > 0
      ? `${additive ? "追加" : "框选"}${formatObjectFamilySelectionSummary(preview.family, preview.ids.length)}`
      : "未命中对象";
  const otherHits =
    preview && preview.family
      ? (["text", "shape", "pen", "number", "effect"] as ObjectSelectionFamily[])
          .filter((family) => family !== preview.family && preview.counts[family] > 0)
          .map((family) => formatObjectFamilySelectionSummary(family, preview.counts[family]))
      : [];
  const secondaryLabel = preview?.family
    ? otherHits.length > 0
      ? `其他命中 ${otherHits.join(" / ")}，当前保持单家族选择`
      : additive
        ? "松开后将追加到当前家族选择"
        : "松开后将替换为当前家族选择"
    : "拖框只会命中文字/图形/画笔/编号/效果对象";

  return (
    <>
      <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
        {annotations.map((annotation) => renderObjectMarqueePreviewAnnotation(annotation, accentColor, fillColor))}
      </svg>
      <div
        className="pointer-events-none absolute border border-dashed"
        style={{
          left: `${rect.x}px`,
          top: `${rect.y}px`,
          width: `${rect.width}px`,
          height: `${rect.height}px`,
          borderColor: accentColor,
          backgroundColor: fillColor,
          boxShadow: `0 0 0 1px ${accentColor}22 inset`,
        }}
      />
      <div
        className="pointer-events-none absolute flex max-w-[min(480px,calc(100vw-24px))] flex-col gap-1"
        style={{
          left: `${Math.max(8, rect.x)}px`,
          top: `${Math.max(8, rect.y - (preview?.family ? 52 : 34))}px`,
        }}
      >
        <div
          className="rounded px-2 py-1 text-[11px] font-medium shadow-[0_10px_24px_rgba(0,0,0,0.28)] backdrop-blur-sm"
          style={{
            backgroundColor: additive ? "rgba(12, 24, 42, 0.82)" : "rgba(0, 0, 0, 0.72)",
            color: accentColor,
            border: `1px solid ${accentColor}55`,
          }}
        >
          {mainLabel}
        </div>
        <div className="rounded bg-black/65 px-2 py-1 text-[11px] text-white/88 shadow-[0_10px_24px_rgba(0,0,0,0.24)] backdrop-blur-sm">
          {secondaryLabel}
        </div>
      </div>
    </>
  );
}

function formatObjectFamilySelectionSummary(family: ObjectSelectionFamily, count: number) {
  const familyLabel =
    family === "text" ? "文字" : family === "shape" ? "图形" : family === "pen" ? "画笔" : family === "number" ? "编号" : "效果";
  const unit = family === "text" || family === "pen" ? "条" : family === "effect" ? "块" : "个";
  return `${familyLabel} ${count} ${unit}`;
}

function renderObjectMarqueePreviewAnnotation(annotation: ObjectSelectionAnnotation, accentColor: string, fillColor: string) {
  if (annotation.kind === "text") {
    const layout = resolveTextAnnotationLayout(annotation);
    const points = layout.corners.map((point) => `${point.x},${point.y}`).join(" ");
    return (
      <polygon
        key={`marquee-preview-${annotation.id}`}
        fill={fillColor}
        points={points}
        stroke={accentColor}
        strokeDasharray="6 4"
        strokeWidth={1.5}
      />
    );
  }

  if (annotation.kind === "pen") {
    const path = buildPath(annotation.points);
    if (!path) {
      return null;
    }

    return (
      <path
        key={`marquee-preview-${annotation.id}`}
        d={path}
        fill="none"
        opacity={0.95}
        stroke={accentColor}
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={Math.max(4, annotation.strokeWidth + 2)}
      />
    );
  }

  if (annotation.kind === "number") {
    const layout = resolveNumberAnnotationLayout(annotation);
    return (
      <g key={`marquee-preview-${annotation.id}`}>
        <circle
          cx={annotation.point.x}
          cy={annotation.point.y}
          fill={fillColor}
          r={layout.radius + 4}
          stroke={accentColor}
          strokeDasharray="6 4"
          strokeWidth={1.5}
        />
      </g>
    );
  }

  if (annotation.kind === "effect") {
    const rect = expandRect(resolveEffectAnnotationBounds(annotation), 2);
    return (
      <rect
        key={`marquee-preview-${annotation.id}`}
        fill={fillColor}
        height={rect.height}
        stroke={accentColor}
        strokeDasharray="6 4"
        strokeWidth={1.5}
        width={rect.width}
        x={rect.x}
        y={rect.y}
      />
    );
  }

  if (annotation.kind === "line") {
    return (
      <line
        key={`marquee-preview-${annotation.id}`}
        opacity={0.95}
        stroke={accentColor}
        strokeLinecap="round"
        strokeWidth={Math.max(4, annotation.strokeWidth + 2)}
        x1={annotation.start.x}
        x2={annotation.end.x}
        y1={annotation.start.y}
        y2={annotation.end.y}
      />
    );
  }

  if (annotation.kind === "arrow") {
    return (
      <g key={`marquee-preview-${annotation.id}`} opacity={0.95}>
        <line
          stroke={accentColor}
          strokeLinecap="round"
          strokeWidth={Math.max(4, annotation.strokeWidth + 2)}
          x1={annotation.start.x}
          x2={annotation.end.x}
          y1={annotation.start.y}
          y2={annotation.end.y}
        />
        <polygon fill={accentColor} points={arrowHead(annotation.start, annotation.end, Math.max(14, (annotation.strokeWidth + 2) * 3))} />
      </g>
    );
  }

  const rect = expandRect(normalizeRect(annotation.start, annotation.end), 2);
  if (annotation.kind === "rect") {
    return (
      <rect
        key={`marquee-preview-${annotation.id}`}
        fill={fillColor}
        height={rect.height}
        stroke={accentColor}
        strokeDasharray="6 4"
        strokeWidth={1.5}
        width={rect.width}
        x={rect.x}
        y={rect.y}
      />
    );
  }

  return (
    <ellipse
      key={`marquee-preview-${annotation.id}`}
      cx={rect.x + rect.width / 2}
      cy={rect.y + rect.height / 2}
      fill={fillColor}
      rx={rect.width / 2}
      ry={rect.height / 2}
      stroke={accentColor}
      strokeDasharray="6 4"
      strokeWidth={1.5}
    />
  );
}

function getShapeKindLabel(kind: ShapeAnnotation["kind"]) {
  switch (kind) {
    case "line":
      return "线条对象";
    case "arrow":
      return "箭头对象";
    case "rect":
      return "矩形对象";
    case "ellipse":
      return "圆形对象";
    default:
      return "图形对象";
  }
}

function SelectionHintBubbles({
  x,
  y,
  items,
  align = "center",
}: {
  x: number;
  y: number;
  items: string[];
  align?: "center" | "left";
}) {
  if (items.length === 0) {
    return null;
  }

  return (
    <div
      className="pointer-events-none absolute flex gap-1"
      style={{
        left: `${x}px`,
        top: `${Math.max(8, y)}px`,
        transform: align === "center" ? "translateX(-50%)" : undefined,
      }}
    >
      {items.map((item) => (
        <div key={item} className="rounded bg-black/65 px-2 py-0.5 text-[11px] text-[#d7ffe9]">
          {item}
        </div>
      ))}
    </div>
  );
}

function SelectionGroupOverlay({ rect, items }: { rect: SelectionRect; items: string[] }) {
  return (
    <>
      <div
        className="pointer-events-none absolute border border-dashed"
        style={{
          left: `${rect.x}px`,
          top: `${rect.y}px`,
          width: `${rect.width}px`,
          height: `${rect.height}px`,
          borderColor: "#00d08f",
          backgroundColor: "rgba(0, 208, 143, 0.04)",
          boxShadow: "0 0 0 1px rgba(0, 208, 143, 0.18) inset",
        }}
      />
      <SelectionHintBubbles items={items} x={rect.x + rect.width / 2} y={rect.y - 34} />
    </>
  );
}

function SelectionStatusBar({ model }: { model: SelectionStatusBarModel }) {
  const palette =
    model.tone === "preview"
      ? {
          border: "rgba(47, 149, 255, 0.42)",
          background: "rgba(10, 24, 44, 0.78)",
          title: "#9cd0ff",
          subtitle: "rgba(230, 242, 255, 0.88)",
          chipBorder: "rgba(47, 149, 255, 0.3)",
          chipBackground: "rgba(47, 149, 255, 0.12)",
          chipText: "#d8ecff",
        }
      : model.tone === "selection"
        ? {
            border: "rgba(0, 208, 143, 0.34)",
            background: "rgba(8, 20, 16, 0.78)",
            title: "#9bf4cf",
            subtitle: "rgba(224, 248, 239, 0.88)",
            chipBorder: "rgba(0, 208, 143, 0.26)",
            chipBackground: "rgba(0, 208, 143, 0.10)",
            chipText: "#d7ffe9",
          }
        : {
            border: "rgba(255, 255, 255, 0.16)",
            background: "rgba(18, 18, 18, 0.68)",
            title: "#f4f4f5",
            subtitle: "rgba(255, 255, 255, 0.76)",
            chipBorder: "rgba(255, 255, 255, 0.12)",
            chipBackground: "rgba(255, 255, 255, 0.06)",
            chipText: "rgba(255, 255, 255, 0.84)",
          };

  return (
    <div
      className="flex flex-col gap-2 rounded border px-3 py-2"
      style={{
        borderColor: palette.border,
        backgroundColor: palette.background,
      }}
    >
      <div className="flex items-center gap-2">
        <span className="text-[12px] font-semibold" style={{ color: palette.title }}>
          {model.title}
        </span>
        <span className="text-[11px]" style={{ color: palette.subtitle }}>
          {model.subtitle}
        </span>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {model.chips.map((chip) => (
          <span
            key={chip}
            className="rounded px-2 py-0.5 text-[11px]"
            style={{
              border: `1px solid ${palette.chipBorder}`,
              backgroundColor: palette.chipBackground,
              color: palette.chipText,
            }}
          >
            {chip}
          </span>
        ))}
      </div>
    </div>
  );
}

function TextSelectionOverlay({
  annotation,
  primary,
  showHint,
}: {
  annotation: TextAnnotation;
  primary: boolean;
  showHint: boolean;
}) {
  const layout = resolveTextAnnotationLayout(annotation);
  const points = layout.corners.map((point) => `${point.x},${point.y}`).join(" ");
  const handlePoint = layout.corners[0];

  return (
    <>
      <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
        <polygon
          fill={primary ? "rgba(0, 208, 143, 0.08)" : "rgba(255, 255, 255, 0.04)"}
          points={points}
          stroke={primary ? "#00d08f" : "rgba(255, 255, 255, 0.75)"}
          strokeDasharray="6 4"
          strokeWidth={1.5}
        />
        <circle cx={handlePoint.x} cy={handlePoint.y} fill={primary ? "#00d08f" : "#ffffff"} r={primary ? 4 : 3} stroke="rgba(0, 0, 0, 0.4)" />
      </svg>
      {showHint ? (
        <SelectionHintBubbles
          items={["双击编辑 / 拖动", "方向键/Shift 微调", "Ctrl+[ / ] 层级", "Ctrl+Shift+[ / ] 置底/置顶"]}
          x={layout.bounds.x + layout.bounds.width / 2}
          y={layout.bounds.y - 34}
        />
      ) : null}
    </>
  );
}

function ShapeSelectionOverlay({
  annotation,
  primary,
  selectedCount,
  showHint,
}: {
  annotation: ShapeAnnotation;
  primary: boolean;
  selectedCount: number;
  showHint: boolean;
}) {
  const bounds = expandRect(resolveShapeAnnotationBounds(annotation), 4);
  const handles = selectedCount === 1 ? resolveShapeHandleDescriptors(annotation) : [];
  const hintX = annotation.kind === "line" || annotation.kind === "arrow" ? (annotation.start.x + annotation.end.x) / 2 : bounds.x + bounds.width / 2;
  const hintY = annotation.kind === "line" || annotation.kind === "arrow" ? Math.min(annotation.start.y, annotation.end.y) : bounds.y;
  const strokeColor = primary ? "#00d08f" : "rgba(255, 255, 255, 0.82)";
  const fillColor = primary ? "rgba(0, 208, 143, 0.08)" : "rgba(255, 255, 255, 0.04)";

  return (
    <>
      <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
        {annotation.kind === "line" || annotation.kind === "arrow" ? (
          <line
            stroke={strokeColor}
            strokeDasharray="6 4"
            strokeWidth={1.5}
            x1={annotation.start.x}
            x2={annotation.end.x}
            y1={annotation.start.y}
            y2={annotation.end.y}
          />
        ) : annotation.kind === "rect" ? (
          <rect
            fill={fillColor}
            height={bounds.height}
            stroke={strokeColor}
            strokeDasharray="6 4"
            strokeWidth={1.5}
            width={bounds.width}
            x={bounds.x}
            y={bounds.y}
          />
        ) : (
          <ellipse
            cx={bounds.x + bounds.width / 2}
            cy={bounds.y + bounds.height / 2}
            fill={fillColor}
            rx={bounds.width / 2}
            ry={bounds.height / 2}
            stroke={strokeColor}
            strokeDasharray="6 4"
            strokeWidth={1.5}
          />
        )}
      </svg>
      {showHint ? (
        <SelectionHintBubbles
          items={[
            annotation.kind === "line" || annotation.kind === "arrow" ? "拖动端点 / 移动" : "拖动句柄 / 移动",
            "Ctrl+C/V/D 复制/粘贴/重复",
            "方向键/Shift 微调",
            "Ctrl+[ / ] 层级",
            "Ctrl+Shift+[ / ] 置底/置顶",
          ]}
          x={hintX}
          y={hintY - 32}
        />
      ) : null}
      {handles.map((handle) => (
        <div
          key={`${annotation.id}-${handle.mode}`}
          className="pointer-events-none absolute h-2.5 w-2.5 rounded-sm border bg-black"
          style={{
            left: `${handle.point.x}px`,
            top: `${handle.point.y}px`,
            transform: "translate(-50%, -50%)",
            borderColor: strokeColor,
          }}
        />
      ))}
    </>
  );
}

function PenSelectionOverlay({
  annotation,
  primary,
  showHint,
}: {
  annotation: PenAnnotation;
  primary: boolean;
  showHint: boolean;
}) {
  const bounds = resolvePenAnnotationBounds(annotation);
  return (
    <>
      <div
        className="pointer-events-none absolute border border-dashed"
        style={{
          left: `${bounds.x}px`,
          top: `${bounds.y}px`,
          width: `${bounds.width}px`,
          height: `${bounds.height}px`,
          borderColor: primary ? "#00d08f" : "rgba(255, 255, 255, 0.75)",
          backgroundColor: primary ? "rgba(0, 208, 143, 0.08)" : "rgba(255, 255, 255, 0.04)",
        }}
      />
      {showHint ? (
        <SelectionHintBubbles
          items={["拖动路径", "Ctrl+C/V/D 复制/粘贴/重复", "方向键/Shift 微调", "Ctrl+[ / ] 层级", "Ctrl+Shift+[ / ] 置底/置顶"]}
          x={bounds.x + bounds.width / 2}
          y={bounds.y - 32}
        />
      ) : null}
    </>
  );
}

function NumberSelectionOverlay({
  annotation,
  primary,
  showHint,
}: {
  annotation: NumberAnnotation;
  primary: boolean;
  showHint: boolean;
}) {
  const layout = resolveNumberAnnotationLayout(annotation);
  const strokeColor = primary ? "#00d08f" : "rgba(255, 255, 255, 0.8)";
  const fillColor = primary ? "rgba(0, 208, 143, 0.08)" : "rgba(255, 255, 255, 0.04)";
  return (
    <>
      <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
        <circle
          cx={annotation.point.x}
          cy={annotation.point.y}
          fill={fillColor}
          r={layout.radius + 3}
          stroke={strokeColor}
          strokeDasharray="6 4"
          strokeWidth={1.5}
        />
        <circle cx={annotation.point.x} cy={annotation.point.y - layout.radius - 3} fill={primary ? "#00d08f" : "#ffffff"} r={primary ? 4 : 3} stroke="rgba(0, 0, 0, 0.4)" />
      </svg>
      {showHint ? (
        <SelectionHintBubbles
          items={[`编号 ${annotation.value}`, "Ctrl+C/V/D 复制/粘贴/重复", "方向键/Shift 微调", "Ctrl+[ / ] 层级", "Ctrl+Shift+[ / ] 置底/置顶"]}
          x={annotation.point.x}
          y={annotation.point.y - layout.radius - 34}
        />
      ) : null}
    </>
  );
}

function EffectSelectionOverlay({
  annotation,
  primary,
  showHandles,
  showHint,
}: {
  annotation: EffectAnnotation;
  primary: boolean;
  showHandles: boolean;
  showHint: boolean;
}) {
  const rect = expandRect(resolveEffectAnnotationBounds(annotation), 2);
  const handles = showHandles ? resolveEffectHandleDescriptors(resolveEffectAnnotationBounds(annotation)) : [];
  return (
    <>
      <div
        className="pointer-events-none absolute border border-dashed"
        style={{
          left: `${rect.x}px`,
          top: `${rect.y}px`,
          width: `${rect.width}px`,
          height: `${rect.height}px`,
          borderColor: primary ? "#00d08f" : "rgba(255, 255, 255, 0.8)",
          backgroundColor: primary ? "rgba(0, 208, 143, 0.08)" : "rgba(255, 255, 255, 0.04)",
        }}
      >
      </div>
      {showHint ? (
        <SelectionHintBubbles
          align="left"
          items={[
            `${annotation.effect === "mosaic" ? "马赛克" : "模糊"} ${Math.round(annotation.intensity)}${showHandles ? "，可拖动/缩放" : ""}`,
            "Ctrl+C/V/D 复制/粘贴/重复",
            "方向键/Shift 微调",
            "Ctrl+[ / ] 层级",
            "Ctrl+Shift+[ / ] 置底/置顶",
          ]}
          x={rect.x}
          y={rect.y - 26}
        />
      ) : null}
      {handles.map((handle) => (
        <div
          key={`${annotation.id}-${handle.mode}`}
          className="pointer-events-none absolute h-2.5 w-2.5 rounded-sm border bg-black"
          style={{
            left: `${handle.point.x}px`,
            top: `${handle.point.y}px`,
            transform: "translate(-50%, -50%)",
            borderColor: "#00d08f",
          }}
        />
      ))}
    </>
  );
}

function renderSnapGuide(guide: SnapGuide, index: number) {
  if (guide.orientation === "vertical") {
    return (
      <line
        key={`snap-v-${index}-${guide.position}`}
        stroke="#00d08f"
        strokeDasharray="6 4"
        strokeWidth={1.5}
        x1={guide.position}
        x2={guide.position}
        y1={guide.start}
        y2={guide.end}
      />
    );
  }

  return (
    <line
      key={`snap-h-${index}-${guide.position}`}
      stroke="#00d08f"
      strokeDasharray="6 4"
      strokeWidth={1.5}
      x1={guide.start}
      x2={guide.end}
      y1={guide.position}
      y2={guide.position}
    />
  );
}

function renderMask(rect: SelectionRect | null) {
  if (!rect) {
    return <div className="pointer-events-none absolute inset-0 bg-black/45" />;
  }

  return (
    <>
      <div className="pointer-events-none absolute inset-x-0 top-0 bg-black/45" style={{ height: `${rect.y}px` }} />
      <div className="pointer-events-none absolute left-0 bg-black/45" style={{ top: `${rect.y}px`, width: `${rect.x}px`, height: `${rect.height}px` }} />
      <div className="pointer-events-none absolute right-0 bg-black/45" style={{ top: `${rect.y}px`, width: `calc(100% - ${rect.x + rect.width}px)`, height: `${rect.height}px` }} />
      <div className="pointer-events-none absolute inset-x-0 bottom-0 bg-black/45" style={{ top: `${rect.y + rect.height}px` }} />
    </>
  );
}

function renderAnnotationSvg(annotation: Annotation, selection: SelectionRect) {
  if (annotation.kind === "fill") {
    return <rect key={annotation.id} fill={annotation.color} fillOpacity={annotation.opacity} height={selection.height} width={selection.width} x={selection.x} y={selection.y} />;
  }

  if (annotation.kind === "effect") {
    return null;
  }

  if (annotation.kind === "number") {
    const layout = resolveNumberAnnotationLayout(annotation);
    return (
      <g key={annotation.id}>
        <circle
          cx={annotation.point.x}
          cy={annotation.point.y}
          fill={layout.fillColor}
          r={layout.radius}
          stroke={layout.borderColor}
          strokeWidth={layout.borderWidth}
        />
        <text
          dominantBaseline="central"
          fill={layout.textColor}
          fontFamily='"MiSans","Segoe UI","PingFang SC",sans-serif'
          fontSize={layout.fontSize}
          fontWeight={700}
          textAnchor="middle"
          x={annotation.point.x}
          y={annotation.point.y}
        >
          {layout.label}
        </text>
      </g>
    );
  }

  if (annotation.kind === "text") {
    const layout = resolveTextAnnotationLayout(annotation);
    const lines = splitTextLines(annotation.text);
    return (
      <g
        key={annotation.id}
        opacity={annotation.opacity}
        transform={annotation.rotation === 0 ? undefined : `rotate(${annotation.rotation} ${layout.center.x} ${layout.center.y})`}
      >
        {layout.boxRect ? (
          <rect
            fill={layout.style.boxFill ?? undefined}
            fillOpacity={layout.style.boxOpacity}
            height={layout.boxRect.height}
            rx={layout.style.radius}
            ry={layout.style.radius}
            width={layout.boxRect.width}
            x={layout.boxRect.x}
            y={layout.boxRect.y}
          />
        ) : null}
        <text
          fill={layout.style.textColor}
          fontSize={annotation.fontSize}
          fontWeight={600}
          paintOrder={layout.style.strokeColor ? "stroke fill" : undefined}
          stroke={layout.style.strokeColor ?? undefined}
          strokeLinejoin={layout.style.strokeColor ? "round" : undefined}
          strokeWidth={layout.style.strokeWidth || undefined}
          x={annotation.point.x}
          y={annotation.point.y}
        >
          {lines.map((line, index) => (
            <tspan key={`${annotation.id}-${index}`} dominantBaseline="hanging" x={annotation.point.x} y={annotation.point.y + index * layout.metrics.lineHeight}>
              {line || " "}
            </tspan>
          ))}
        </text>
      </g>
    );
  }

  if (annotation.kind === "pen") {
    const path = buildPath(annotation.points);
    if (!path) return null;
    return <path key={annotation.id} d={path} fill="none" stroke={annotation.color} strokeLinecap="round" strokeLinejoin="round" strokeWidth={annotation.strokeWidth} />;
  }

  if (annotation.kind === "line") {
    return <line key={annotation.id} stroke={annotation.color} strokeLinecap="round" strokeWidth={annotation.strokeWidth} x1={annotation.start.x} x2={annotation.end.x} y1={annotation.start.y} y2={annotation.end.y} />;
  }

  if (annotation.kind === "arrow") {
    const head = arrowHead(annotation.start, annotation.end, Math.max(12, annotation.strokeWidth * 3));
    return (
      <g key={annotation.id}>
        <line stroke={annotation.color} strokeLinecap="round" strokeWidth={annotation.strokeWidth} x1={annotation.start.x} x2={annotation.end.x} y1={annotation.start.y} y2={annotation.end.y} />
        <polygon fill={annotation.color} points={head} />
      </g>
    );
  }

  const rect = normalizeRect(annotation.start, annotation.end);
  if (annotation.kind === "rect") {
    return <rect key={annotation.id} fill="none" height={rect.height} stroke={annotation.color} strokeWidth={annotation.strokeWidth} width={rect.width} x={rect.x} y={rect.y} />;
  }

  return <ellipse key={annotation.id} cx={rect.x + rect.width / 2} cy={rect.y + rect.height / 2} fill="none" rx={rect.width / 2} ry={rect.height / 2} stroke={annotation.color} strokeWidth={annotation.strokeWidth} />;
}

async function renderAnnotatedSelectionDataUrl(
  render: ScreenshotSelectionRenderView,
  selection: SelectionRect,
  annotations: Annotation[],
) {
  const image = await loadImage(render.imageDataUrl);
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(render.width));
  canvas.height = Math.max(1, Math.round(render.height));
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("CANVAS_CONTEXT_UNAVAILABLE");
  }

  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  if (annotations.length === 0) {
    return canvas.toDataURL("image/png");
  }

  if (render.tiles.length > 0) {
    drawAnnotationsOnSelectionRender(context, image, render, selection, annotations);
    return canvas.toDataURL("image/png");
  }

  const scale = render.scaleFactor > 0 ? render.scaleFactor : 1;
  drawAnnotationsOnCanvas(context, annotations, selection, scale, scale, canvas.width, canvas.height);
  return canvas.toDataURL("image/png");
}

function drawAnnotationsOnSelectionRender(
  context: CanvasRenderingContext2D,
  baseImage: HTMLImageElement,
  render: ScreenshotSelectionRenderView,
  selection: SelectionRect,
  annotations: Annotation[],
) {
  const sortedTiles = [...render.tiles].sort((left, right) => {
    if (left.outputY !== right.outputY) {
      return left.outputY - right.outputY;
    }
    return left.outputX - right.outputX;
  });

  for (const tile of sortedTiles) {
    const outputWidth = Math.max(1, Math.round(tile.outputWidth));
    const outputHeight = Math.max(1, Math.round(tile.outputHeight));
    const logicalWidth = Math.max(tile.logicalWidth, 1);
    const logicalHeight = Math.max(tile.logicalHeight, 1);
    const tileContext = createCanvasContext(outputWidth, outputHeight);
    if (!tileContext) {
      continue;
    }

    tileContext.drawImage(
      baseImage,
      tile.outputX,
      tile.outputY,
      outputWidth,
      outputHeight,
      0,
      0,
      outputWidth,
      outputHeight,
    );

    drawAnnotationsOnCanvas(
      tileContext,
      annotations,
      {
        x: selection.x + tile.logicalX,
        y: selection.y + tile.logicalY,
        width: logicalWidth,
        height: logicalHeight,
      },
      outputWidth / logicalWidth,
      outputHeight / logicalHeight,
      outputWidth,
      outputHeight,
    );

    context.drawImage(tileContext.canvas, tile.outputX, tile.outputY);
  }
}

function drawEffectPreviewLayer(context: CanvasRenderingContext2D, image: CanvasImageSource, width: number, height: number, annotations: EffectAnnotation[]) {
  context.clearRect(0, 0, width, height);
  context.drawImage(image, 0, 0, width, height);
  const viewport = { x: 0, y: 0, width, height };

  for (const annotation of annotations) {
    applyEffectAnnotationToCanvas(context, annotation, viewport, 1, 1, 1);
  }
}

function drawAnnotationsOnCanvas(context: CanvasRenderingContext2D, annotations: Annotation[], selection: SelectionRect, scaleX: number, scaleY: number, canvasWidth: number, canvasHeight: number) {
  const averageScale = (scaleX + scaleY) / 2;
  const effectAnnotations = annotations.filter((annotation): annotation is EffectAnnotation => annotation.kind === "effect");
  const layeredAnnotations = annotations.filter((annotation) => annotation.kind !== "effect");

  for (const annotation of effectAnnotations) {
    applyEffectAnnotationToCanvas(context, annotation, selection, scaleX, scaleY, averageScale);
  }

  for (const annotation of layeredAnnotations) {
    if (annotation.kind === "fill") {
      context.fillStyle = toRgba(annotation.color, annotation.opacity);
      context.fillRect(0, 0, canvasWidth, canvasHeight);
      continue;
    }

    if (annotation.kind === "number") {
      const point = mapPoint(annotation.point, selection, scaleX, scaleY);
      const layout = resolveNumberAnnotationLayout({
        ...annotation,
        size: annotation.size * averageScale,
        point,
      });

      context.save();
      context.beginPath();
      context.fillStyle = layout.fillColor;
      context.arc(point.x, point.y, layout.radius, 0, Math.PI * 2);
      context.fill();
      context.strokeStyle = layout.borderColor;
      context.lineWidth = layout.borderWidth;
      context.stroke();
      context.fillStyle = layout.textColor;
      context.font = `700 ${layout.fontSize}px "MiSans","Segoe UI","PingFang SC",sans-serif`;
      context.textAlign = "center";
      context.textBaseline = "middle";
      context.fillText(layout.label, point.x, point.y);
      context.restore();
      continue;
    }

    if (annotation.kind === "text") {
      const point = mapPoint(annotation.point, selection, scaleX, scaleY);
      const scaledAnnotation: TextAnnotation = {
        ...annotation,
        fontSize: annotation.fontSize * averageScale,
        point,
      };
      const layout = resolveTextAnnotationLayout(scaledAnnotation);

      context.save();
      context.globalAlpha = clamp(annotation.opacity, 0, 1);
      if (annotation.rotation !== 0) {
        context.translate(layout.center.x, layout.center.y);
        context.rotate((annotation.rotation * Math.PI) / 180);
        context.translate(-layout.center.x, -layout.center.y);
      }

      if (layout.boxRect && layout.style.boxFill) {
        drawRoundedRect(
          context,
          layout.boxRect.x,
          layout.boxRect.y,
          layout.boxRect.width,
          layout.boxRect.height,
          layout.style.radius,
          toRgba(layout.style.boxFill, layout.style.boxOpacity),
        );
      }

      context.fillStyle = layout.style.textColor;
      context.font = `600 ${Math.max(10, annotation.fontSize * averageScale)}px "MiSans","Segoe UI","PingFang SC",sans-serif`;
      context.textBaseline = "top";
      if (layout.style.strokeColor) {
        context.strokeStyle = layout.style.strokeColor;
        context.lineWidth = layout.style.strokeWidth;
        context.lineJoin = "round";
      }
      for (const [index, line] of splitTextLines(annotation.text).entries()) {
        const lineText = line || " ";
        const y = point.y + index * layout.metrics.lineHeight;
        if (layout.style.strokeColor) {
          context.strokeText(lineText, point.x, y);
        }
        context.fillText(lineText, point.x, y);
      }
      context.restore();
      continue;
    }

    if (annotation.kind === "pen") {
      const points = annotation.points.map((point) => mapPoint(point, selection, scaleX, scaleY));
      if (points.length < 2) continue;
      context.beginPath();
      context.strokeStyle = annotation.color;
      context.lineWidth = Math.max(1, annotation.strokeWidth * averageScale);
      context.lineCap = "round";
      context.lineJoin = "round";
      context.moveTo(points[0].x, points[0].y);
      for (let index = 1; index < points.length; index += 1) {
        context.lineTo(points[index].x, points[index].y);
      }
      context.stroke();
      continue;
    }

    const start = mapPoint(annotation.start, selection, scaleX, scaleY);
    const end = mapPoint(annotation.end, selection, scaleX, scaleY);
    context.strokeStyle = annotation.color;
    context.fillStyle = annotation.color;
    context.lineWidth = Math.max(1, annotation.strokeWidth * averageScale);
    context.lineCap = "round";
    context.lineJoin = "round";

    if (annotation.kind === "line") {
      context.beginPath();
      context.moveTo(start.x, start.y);
      context.lineTo(end.x, end.y);
      context.stroke();
      continue;
    }

    if (annotation.kind === "arrow") {
      context.beginPath();
      context.moveTo(start.x, start.y);
      context.lineTo(end.x, end.y);
      context.stroke();

      const points = arrowHead(start, end, Math.max(12, annotation.strokeWidth * averageScale * 3)).split(" ").map((item) => item.split(",").map(Number));
      context.beginPath();
      context.moveTo(points[0][0], points[0][1]);
      context.lineTo(points[1][0], points[1][1]);
      context.lineTo(points[2][0], points[2][1]);
      context.closePath();
      context.fill();
      continue;
    }

    const rect = normalizeRect(start, end);
    if (annotation.kind === "rect") {
      context.strokeRect(rect.x, rect.y, rect.width, rect.height);
      continue;
    }

    context.beginPath();
    context.ellipse(rect.x + rect.width / 2, rect.y + rect.height / 2, rect.width / 2, rect.height / 2, 0, 0, Math.PI * 2);
    context.stroke();
  }
}

function applyEffectAnnotationToCanvas(context: CanvasRenderingContext2D, annotation: EffectAnnotation, selection: SelectionRect, scaleX: number, scaleY: number, averageScale: number) {
  const rect = normalizeRect(annotation.start, annotation.end);
  const mappedRect = {
    x: (rect.x - selection.x) * scaleX,
    y: (rect.y - selection.y) * scaleY,
    width: rect.width * scaleX,
    height: rect.height * scaleY,
  };

  applyRegionEffect(context, mappedRect, annotation.effect, annotation.intensity * averageScale);
}

function applyRegionEffect(context: CanvasRenderingContext2D, rect: SelectionRect, effect: EffectKind, intensity: number) {
  const target = clampRectToCanvas(rect, context.canvas.width, context.canvas.height);
  if (!target) {
    return;
  }

  if (effect === "mosaic") {
    applyMosaicRegionEffect(context, target, intensity);
    return;
  }

  applyBlurRegionEffect(context, target, intensity);
}

function applyMosaicRegionEffect(context: CanvasRenderingContext2D, rect: SelectionRect, intensity: number) {
  const sourceContext = createCanvasContext(Math.max(1, Math.round(rect.width)), Math.max(1, Math.round(rect.height)));
  if (!sourceContext) return;

  sourceContext.drawImage(
    context.canvas,
    rect.x,
    rect.y,
    rect.width,
    rect.height,
    0,
    0,
    rect.width,
    rect.height,
  );

  const blockSize = clampNumber(Math.round(intensity), 4, 64);
  const scaledWidth = Math.max(1, Math.round(rect.width / blockSize));
  const scaledHeight = Math.max(1, Math.round(rect.height / blockSize));
  const pixelContext = createCanvasContext(scaledWidth, scaledHeight);
  if (!pixelContext) return;

  pixelContext.drawImage(sourceContext.canvas, 0, 0, rect.width, rect.height, 0, 0, scaledWidth, scaledHeight);
  sourceContext.clearRect(0, 0, rect.width, rect.height);
  sourceContext.imageSmoothingEnabled = false;
  sourceContext.drawImage(pixelContext.canvas, 0, 0, scaledWidth, scaledHeight, 0, 0, rect.width, rect.height);
  sourceContext.imageSmoothingEnabled = true;
  context.drawImage(sourceContext.canvas, rect.x, rect.y);
}

function applyBlurRegionEffect(context: CanvasRenderingContext2D, rect: SelectionRect, intensity: number) {
  const blurRadius = clampNumber(intensity, 2, 36);
  const padding = Math.max(6, Math.ceil(blurRadius * 2));
  const sampleRect = clampRectToCanvas(
    {
      x: rect.x - padding,
      y: rect.y - padding,
      width: rect.width + padding * 2,
      height: rect.height + padding * 2,
    },
    context.canvas.width,
    context.canvas.height,
  );
  if (!sampleRect) return;

  const sampleContext = createCanvasContext(Math.max(1, Math.round(sampleRect.width)), Math.max(1, Math.round(sampleRect.height)));
  const outputContext = createCanvasContext(Math.max(1, Math.round(sampleRect.width)), Math.max(1, Math.round(sampleRect.height)));
  if (!sampleContext || !outputContext) return;

  sampleContext.drawImage(
    context.canvas,
    sampleRect.x,
    sampleRect.y,
    sampleRect.width,
    sampleRect.height,
    0,
    0,
    sampleRect.width,
    sampleRect.height,
  );

  outputContext.filter = `blur(${blurRadius}px)`;
  outputContext.drawImage(sampleContext.canvas, 0, 0);
  outputContext.filter = "none";

  const cropX = rect.x - sampleRect.x;
  const cropY = rect.y - sampleRect.y;
  context.drawImage(outputContext.canvas, cropX, cropY, rect.width, rect.height, rect.x, rect.y, rect.width, rect.height);
}

function mapPoint(point: Point, selection: SelectionRect, scaleX: number, scaleY: number): Point {
  return {
    x: (point.x - selection.x) * scaleX,
    y: (point.y - selection.y) * scaleY,
  };
}

function getNextNumberValue(annotations: Annotation[]) {
  let maxValue = 0;
  for (const annotation of annotations) {
    if (annotation.kind !== "number") {
      continue;
    }
    maxValue = Math.max(maxValue, annotation.value);
  }
  return maxValue + 1;
}

function clampNumberAnnotationToSelection(annotation: NumberAnnotation, selection: SelectionRect): NumberAnnotation {
  const layout = resolveNumberAnnotationLayout(annotation);
  const minX = selection.x + layout.radius;
  const minY = selection.y + layout.radius;
  const maxX = Math.max(minX, selection.x + selection.width - layout.radius);
  const maxY = Math.max(minY, selection.y + selection.height - layout.radius);
  const nextPoint = {
    x: clamp(annotation.point.x, minX, maxX),
    y: clamp(annotation.point.y, minY, maxY),
  };

  if (nextPoint.x === annotation.point.x && nextPoint.y === annotation.point.y) {
    return annotation;
  }

  return {
    ...annotation,
    point: nextPoint,
  };
}

function buildDisplayAnnotations(
  annotations: Annotation[],
  textEditor: TextEditorState | null,
  textDrag: TextDragState | null,
  shapeTransform: ShapeTransformState | null,
  shapeGroupDrag: ShapeGroupDragState | null,
  penTransform: PenTransformState | null,
  penGroupDrag: PenGroupDragState | null,
  effectTransform: EffectTransformState | null,
  numberDrag: NumberDragState | null,
  numberGroupDrag: NumberGroupDragState | null,
  effectGroupDrag: EffectGroupDragState | null,
  mixedGroupDrag: MixedGroupDragState | null,
  hiddenAnnotationIds: string[],
) {
  let next = cloneAnnotations(annotations);
  const hiddenSet = hiddenAnnotationIds.length > 0 ? new Set(hiddenAnnotationIds) : null;

  if (hiddenSet) {
    next = next.filter((annotation) => !hiddenSet.has(annotation.id));
  }

  if (textEditor?.sourceAnnotationId) {
    next = next.filter((annotation) => annotation.id !== textEditor.sourceAnnotationId);
  }

  if (textDrag) {
    next = next.map((annotation) => {
      if (annotation.kind !== "text" || !textDrag.ids.includes(annotation.id)) {
        return annotation;
      }
      const originPoint = textDrag.originPoints[annotation.id] ?? annotation.point;
      return {
        ...annotation,
        point: {
          x: originPoint.x + textDrag.delta.x,
          y: originPoint.y + textDrag.delta.y,
        },
      };
    });
  }

  if (shapeTransform) {
    next = next.map((annotation) => {
      if ((annotation.kind !== "line" && annotation.kind !== "rect" && annotation.kind !== "ellipse" && annotation.kind !== "arrow") || annotation.id !== shapeTransform.id) {
        return annotation;
      }
      return shapeTransform.previewAnnotation;
    });
  }

  if (shapeGroupDrag) {
    next = next.map((annotation) => {
      if ((annotation.kind !== "line" && annotation.kind !== "rect" && annotation.kind !== "ellipse" && annotation.kind !== "arrow") || !shapeGroupDrag.ids.includes(annotation.id)) {
        return annotation;
      }
      const originAnnotation = shapeGroupDrag.originAnnotations[annotation.id] ?? annotation;
      return offsetShapeAnnotation(originAnnotation, shapeGroupDrag.delta);
    });
  }

  if (penTransform) {
    next = next.map((annotation) => {
      if (annotation.kind !== "pen" || annotation.id !== penTransform.id) {
        return annotation;
      }
      return penTransform.previewAnnotation;
    });
  }

  if (penGroupDrag) {
    next = next.map((annotation) => {
      if (annotation.kind !== "pen") {
        return annotation;
      }
      const originAnnotation = penGroupDrag.originAnnotations[annotation.id];
      if (!originAnnotation) {
        return annotation;
      }
      return offsetPenAnnotation(originAnnotation, penGroupDrag.delta);
    });
  }

  if (effectTransform) {
    next = next.map((annotation) => {
      if (annotation.kind !== "effect" || annotation.id !== effectTransform.id) {
        return annotation;
      }
      return createEffectAnnotationWithBounds(annotation, effectTransform.previewBounds);
    });
  }

  if (effectGroupDrag) {
    next = next.map((annotation) => {
      if (annotation.kind !== "effect" || !effectGroupDrag.ids.includes(annotation.id)) {
        return annotation;
      }
      const originBounds = effectGroupDrag.originBounds[annotation.id] ?? resolveEffectAnnotationBounds(annotation);
      return createEffectAnnotationWithBounds(annotation, offsetRect(originBounds, effectGroupDrag.delta));
    });
  }

  if (numberDrag) {
    next = next.map((annotation) => {
      if (annotation.kind !== "number" || annotation.id !== numberDrag.id) {
        return annotation;
      }
      return numberDrag.previewAnnotation;
    });
  }

  if (numberGroupDrag) {
    next = next.map((annotation) => {
      if (annotation.kind !== "number" || !numberGroupDrag.ids.includes(annotation.id)) {
        return annotation;
      }
      const originPoint = numberGroupDrag.originPoints[annotation.id] ?? annotation.point;
      return {
        ...annotation,
        point: {
          x: originPoint.x + numberGroupDrag.delta.x,
          y: originPoint.y + numberGroupDrag.delta.y,
        },
      };
    });
  }

  if (mixedGroupDrag) {
    next = next.map((annotation) => {
      const originAnnotation = mixedGroupDrag.originAnnotations[annotation.id];
      if (!originAnnotation) {
        return annotation;
      }
      return offsetObjectSelectionAnnotation(originAnnotation, mixedGroupDrag.delta);
    });
  }

  return next;
}

function moveAnnotationLayer(
  annotations: Annotation[],
  targetIds: string[],
  direction: "forward" | "backward" | "front" | "back",
): Annotation[] | null {
  if (targetIds.length === 0) {
    return null;
  }

  const selectedSet = new Set(targetIds);
  if (direction === "front" || direction === "back") {
    const selectedItems = annotations.filter((annotation) => selectedSet.has(annotation.id));
    const remainingItems = annotations.filter((annotation) => !selectedSet.has(annotation.id));
    if (selectedItems.length === 0) {
      return null;
    }

    const nextAnnotations = cloneAnnotations(direction === "front" ? [...remainingItems, ...selectedItems] : [...selectedItems, ...remainingItems]);
    const changed = nextAnnotations.some((annotation, index) => annotation.id !== annotations[index]?.id);
    return changed ? nextAnnotations : null;
  }

  const nextAnnotations = cloneAnnotations(annotations);
  let changed = false;

  if (direction === "forward") {
    for (let index = nextAnnotations.length - 2; index >= 0; index -= 1) {
      if (!selectedSet.has(nextAnnotations[index].id) || selectedSet.has(nextAnnotations[index + 1].id)) {
        continue;
      }
      [nextAnnotations[index], nextAnnotations[index + 1]] = [nextAnnotations[index + 1], nextAnnotations[index]];
      changed = true;
    }
  } else {
    for (let index = 1; index < nextAnnotations.length; index += 1) {
      if (!selectedSet.has(nextAnnotations[index].id) || selectedSet.has(nextAnnotations[index - 1].id)) {
        continue;
      }
      [nextAnnotations[index - 1], nextAnnotations[index]] = [nextAnnotations[index], nextAnnotations[index - 1]];
      changed = true;
    }
  }

  return changed ? nextAnnotations : null;
}

function cloneAnnotations(annotations: Annotation[]) {
  return annotations.map((annotation) => cloneAnnotation(annotation));
}

function cloneAnnotation(annotation: Annotation): Annotation {
  if (annotation.kind === "pen") {
    return {
      ...annotation,
      points: annotation.points.map((point) => ({ ...point })),
    };
  }

  if (annotation.kind === "text") {
    return {
      ...annotation,
      point: { ...annotation.point },
    };
  }

  if (annotation.kind === "number") {
    return {
      ...annotation,
      point: { ...annotation.point },
    };
  }

  if (annotation.kind === "fill") {
    return { ...annotation };
  }

  return {
    ...annotation,
    start: { ...annotation.start },
    end: { ...annotation.end },
  };
}

function buildPath(points: Point[]) {
  if (points.length < 2) return "";
  const parts = [`M ${points[0].x} ${points[0].y}`];
  for (let index = 1; index < points.length; index += 1) {
    parts.push(`L ${points[index].x} ${points[index].y}`);
  }
  return parts.join(" ");
}

function arrowHead(start: Point, end: Point, size: number) {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const left = {
    x: end.x - size * Math.cos(angle - Math.PI / 6),
    y: end.y - size * Math.sin(angle - Math.PI / 6),
  };
  const right = {
    x: end.x - size * Math.cos(angle + Math.PI / 6),
    y: end.y - size * Math.sin(angle + Math.PI / 6),
  };
  return `${end.x},${end.y} ${left.x},${left.y} ${right.x},${right.y}`;
}

function normalizeRect(start: Point, end: Point): SelectionRect {
  const x = Math.min(start.x, end.x);
  const y = Math.min(start.y, end.y);
  const right = Math.max(start.x, end.x);
  const bottom = Math.max(start.y, end.y);
  return {
    x,
    y,
    width: right - x,
    height: bottom - y,
  };
}

function isPointInRect(point: Point, rect: SelectionRect) {
  return point.x >= rect.x && point.x <= rect.x + rect.width && point.y >= rect.y && point.y <= rect.y + rect.height;
}

function findTextAnnotationById(annotations: Annotation[], id: string) {
  const annotation = annotations.find((item) => item.kind === "text" && item.id === id);
  return annotation && annotation.kind === "text" ? annotation : null;
}

function findShapeAnnotationById(annotations: Annotation[], id: string) {
  const annotation = annotations.find(
    (item) => (item.kind === "line" || item.kind === "rect" || item.kind === "ellipse" || item.kind === "arrow") && item.id === id,
  );
  return annotation && (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow")
    ? annotation
    : null;
}

function findPenAnnotationById(annotations: Annotation[], id: string) {
  const annotation = annotations.find((item) => item.kind === "pen" && item.id === id);
  return annotation && annotation.kind === "pen" ? annotation : null;
}

function findNumberAnnotationById(annotations: Annotation[], id: string) {
  const annotation = annotations.find((item) => item.kind === "number" && item.id === id);
  return annotation && annotation.kind === "number" ? annotation : null;
}

function findEffectAnnotationById(annotations: Annotation[], id: string) {
  const annotation = annotations.find((item) => item.kind === "effect" && item.id === id);
  return annotation && annotation.kind === "effect" ? annotation : null;
}

function findTextAnnotationAtPoint(annotations: Annotation[], point: Point) {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (annotation.kind !== "text") continue;
    const layout = resolveTextAnnotationLayout(annotation);
    const hitPoint = annotation.rotation === 0 ? point : rotatePoint(point, layout.center, -annotation.rotation);
    if (isPointInRect(hitPoint, expandRect(layout.frame, 6))) {
      return annotation;
    }
  }
  return null;
}

function resolveShapeAnnotationBounds(annotation: ShapeAnnotation) {
  return normalizeRect(annotation.start, annotation.end);
}

function resolvePenAnnotationBounds(annotation: PenAnnotation) {
  if (annotation.points.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const xs = annotation.points.map((point) => point.x);
  const ys = annotation.points.map((point) => point.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  const maxX = Math.max(...xs);
  const maxY = Math.max(...ys);
  const padding = annotation.strokeWidth / 2 + 4;
  return {
    x: minX - padding,
    y: minY - padding,
    width: maxX - minX + padding * 2,
    height: maxY - minY + padding * 2,
  };
}

function clampPointToSelection(point: Point, selection: SelectionRect): Point {
  return {
    x: clamp(point.x, selection.x, selection.x + selection.width),
    y: clamp(point.y, selection.y, selection.y + selection.height),
  };
}

function offsetShapeAnnotation(annotation: ShapeAnnotation, delta: Point): ShapeAnnotation {
  return {
    ...annotation,
    start: {
      x: annotation.start.x + delta.x,
      y: annotation.start.y + delta.y,
    },
    end: {
      x: annotation.end.x + delta.x,
      y: annotation.end.y + delta.y,
    },
  };
}

function offsetPenAnnotation(annotation: PenAnnotation, delta: Point): PenAnnotation {
  return {
    ...annotation,
    points: annotation.points.map((point) => ({
      x: point.x + delta.x,
      y: point.y + delta.y,
    })),
  };
}

function offsetObjectSelectionAnnotation(annotation: ObjectSelectionAnnotation, delta: Point): ObjectSelectionAnnotation {
  if (annotation.kind === "text") {
    return {
      ...annotation,
      point: {
        x: annotation.point.x + delta.x,
        y: annotation.point.y + delta.y,
      },
    };
  }

  if (annotation.kind === "pen") {
    return offsetPenAnnotation(annotation, delta);
  }

  if (annotation.kind === "number") {
    return {
      ...annotation,
      point: {
        x: annotation.point.x + delta.x,
        y: annotation.point.y + delta.y,
      },
    };
  }

  if (annotation.kind === "effect") {
    return createEffectAnnotationWithBounds(annotation, offsetRect(resolveEffectAnnotationBounds(annotation), delta));
  }

  return offsetShapeAnnotation(annotation, delta);
}

function createShapeAnnotationWithBounds(annotation: ShapeAnnotation, bounds: SelectionRect): ShapeAnnotation {
  return {
    ...annotation,
    start: { x: bounds.x, y: bounds.y },
    end: { x: bounds.x + bounds.width, y: bounds.y + bounds.height },
  };
}

function distanceToSegment(point: Point, start: Point, end: Point) {
  const lengthSquared = (end.x - start.x) ** 2 + (end.y - start.y) ** 2;
  if (lengthSquared <= 0.0001) {
    return distance(point, start);
  }

  const projection = ((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y)) / lengthSquared;
  const ratio = clamp(projection, 0, 1);
  const nearest = {
    x: start.x + (end.x - start.x) * ratio,
    y: start.y + (end.y - start.y) * ratio,
  };
  return distance(point, nearest);
}

function isPointNearRectOutline(point: Point, rect: SelectionRect, tolerance: number) {
  const expanded = expandRect(rect, tolerance);
  if (!isPointInRect(point, expanded)) {
    return false;
  }

  const innerWidth = Math.max(0, rect.width - tolerance * 2);
  const innerHeight = Math.max(0, rect.height - tolerance * 2);
  if (innerWidth <= 0 || innerHeight <= 0) {
    return true;
  }

  const inner = {
    x: rect.x + tolerance,
    y: rect.y + tolerance,
    width: innerWidth,
    height: innerHeight,
  };
  return !isPointInRect(point, inner);
}

function isPointNearEllipseOutline(point: Point, rect: SelectionRect, tolerance: number) {
  const centerX = rect.x + rect.width / 2;
  const centerY = rect.y + rect.height / 2;
  const outerRx = rect.width / 2 + tolerance;
  const outerRy = rect.height / 2 + tolerance;
  if (outerRx <= 0 || outerRy <= 0) {
    return distance(point, { x: centerX, y: centerY }) <= tolerance;
  }

  const outerEquation = ((point.x - centerX) ** 2) / (outerRx ** 2) + ((point.y - centerY) ** 2) / (outerRy ** 2);
  if (outerEquation > 1) {
    return false;
  }

  const innerRx = rect.width / 2 - tolerance;
  const innerRy = rect.height / 2 - tolerance;
  if (innerRx <= 0 || innerRy <= 0) {
    return true;
  }

  const innerEquation = ((point.x - centerX) ** 2) / (innerRx ** 2) + ((point.y - centerY) ** 2) / (innerRy ** 2);
  return innerEquation >= 1;
}

function isPointInEllipse(point: Point, rect: SelectionRect, tolerance = 0) {
  const centerX = rect.x + rect.width / 2;
  const centerY = rect.y + rect.height / 2;
  const radiusX = rect.width / 2 + tolerance;
  const radiusY = rect.height / 2 + tolerance;
  if (radiusX <= 0 || radiusY <= 0) {
    return distance(point, { x: centerX, y: centerY }) <= Math.max(tolerance, 1);
  }

  const equation = ((point.x - centerX) ** 2) / (radiusX ** 2) + ((point.y - centerY) ** 2) / (radiusY ** 2);
  return equation <= 1;
}

function findShapeAnnotationAtPoint(annotations: Annotation[], point: Point) {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (annotation.kind !== "line" && annotation.kind !== "rect" && annotation.kind !== "ellipse" && annotation.kind !== "arrow") {
      continue;
    }

    const tolerance = Math.max(8, annotation.strokeWidth + 5);
    if (annotation.kind === "line" || annotation.kind === "arrow") {
      if (distanceToSegment(point, annotation.start, annotation.end) <= tolerance) {
        return annotation;
      }
      continue;
    }

    const bounds = resolveShapeAnnotationBounds(annotation);
    if (annotation.kind === "rect") {
      if (isPointNearRectOutline(point, bounds, tolerance) || isPointInRect(point, bounds)) {
        return annotation;
      }
      continue;
    }
    if (annotation.kind === "ellipse") {
      if (isPointNearEllipseOutline(point, bounds, tolerance) || isPointInEllipse(point, bounds)) {
        return annotation;
      }
      continue;
    }
  }
  return null;
}

function findPenAnnotationAtPoint(annotations: Annotation[], point: Point) {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (annotation.kind !== "pen") {
      continue;
    }

    const tolerance = Math.max(8, annotation.strokeWidth + 4);
    if (annotation.points.length === 1) {
      if (distance(annotation.points[0], point) <= tolerance) {
        return annotation;
      }
      continue;
    }

    for (let pointIndex = 1; pointIndex < annotation.points.length; pointIndex += 1) {
      if (distanceToSegment(point, annotation.points[pointIndex - 1], annotation.points[pointIndex]) <= tolerance) {
        return annotation;
      }
    }
  }
  return null;
}

function resolveShapeHandleDescriptors(annotation: ShapeAnnotation): ShapeHandleDescriptor[] {
  if (annotation.kind === "line" || annotation.kind === "arrow") {
    return [
      { mode: "start", point: annotation.start, cursor: "grab" },
      { mode: "end", point: annotation.end, cursor: "grab" },
    ];
  }

  return resolveEffectHandleDescriptors(resolveShapeAnnotationBounds(annotation)).map((handle) => ({
    mode: handle.mode,
    point: handle.point,
    cursor: handle.cursor,
  }));
}

function findShapeHandleAtPoint(annotation: ShapeAnnotation, point: Point, radius = 8): Exclude<ShapeTransformMode, "move"> | null {
  const handles = resolveShapeHandleDescriptors(annotation);
  for (const handle of handles) {
    if (distance(handle.point, point) <= radius) {
      return handle.mode;
    }
  }
  return null;
}

function resolveShapeTransformAnnotation(
  mode: ShapeTransformMode,
  originAnnotation: ShapeAnnotation,
  startPointer: Point,
  currentPointer: Point,
  selection: SelectionRect,
): ShapeAnnotation {
  if (mode === "move") {
    const delta = clampGroupDeltaToSelection(
      {
        x: currentPointer.x - startPointer.x,
        y: currentPointer.y - startPointer.y,
      },
      resolveShapeAnnotationBounds(originAnnotation),
      selection,
    );
    return offsetShapeAnnotation(originAnnotation, delta);
  }

  if (mode === "start" || mode === "end") {
    const nextPoint = clampPointToSelection(currentPointer, selection);
    return {
      ...originAnnotation,
      [mode]: nextPoint,
    };
  }

  const nextBounds = resolveEffectTransformBounds(
    mode,
    resolveShapeAnnotationBounds(originAnnotation),
    startPointer,
    currentPointer,
    selection,
  );
  return createShapeAnnotationWithBounds(originAnnotation, nextBounds);
}

function findNumberAnnotationAtPoint(annotations: Annotation[], point: Point) {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (annotation.kind !== "number") continue;
    const layout = resolveNumberAnnotationLayout(annotation);
    if (distance(annotation.point, point) <= layout.radius + 6) {
      return annotation;
    }
  }
  return null;
}

function resolveEffectAnnotationBounds(annotation: EffectAnnotation) {
  return normalizeRect(annotation.start, annotation.end);
}

function createEffectAnnotationWithBounds(annotation: EffectAnnotation, bounds: SelectionRect): EffectAnnotation {
  return {
    ...annotation,
    start: { x: bounds.x, y: bounds.y },
    end: { x: bounds.x + bounds.width, y: bounds.y + bounds.height },
  };
}

function resolveEffectHandleDescriptors(bounds: SelectionRect): EffectHandleDescriptor[] {
  const left = bounds.x;
  const right = bounds.x + bounds.width;
  const top = bounds.y;
  const bottom = bounds.y + bounds.height;
  const centerX = bounds.x + bounds.width / 2;
  const centerY = bounds.y + bounds.height / 2;

  return [
    { mode: "nw", point: { x: left, y: top }, cursor: "nwse-resize" },
    { mode: "n", point: { x: centerX, y: top }, cursor: "ns-resize" },
    { mode: "ne", point: { x: right, y: top }, cursor: "nesw-resize" },
    { mode: "e", point: { x: right, y: centerY }, cursor: "ew-resize" },
    { mode: "se", point: { x: right, y: bottom }, cursor: "nwse-resize" },
    { mode: "s", point: { x: centerX, y: bottom }, cursor: "ns-resize" },
    { mode: "sw", point: { x: left, y: bottom }, cursor: "nesw-resize" },
    { mode: "w", point: { x: left, y: centerY }, cursor: "ew-resize" },
  ];
}

function findEffectHandleAtPoint(annotation: EffectAnnotation, point: Point, radius = 8): Exclude<EffectTransformMode, "move"> | null {
  const handles = resolveEffectHandleDescriptors(resolveEffectAnnotationBounds(annotation));
  for (const handle of handles) {
    if (distance(handle.point, point) <= radius) {
      return handle.mode;
    }
  }
  return null;
}

function findEffectAnnotationAtPoint(annotations: Annotation[], point: Point) {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (annotation.kind !== "effect") continue;
    if (isPointInRect(point, expandRect(resolveEffectAnnotationBounds(annotation), 6))) {
      return annotation;
    }
  }
  return null;
}

function doRectsIntersect(left: SelectionRect, right: SelectionRect) {
  return (
    left.x <= right.x + right.width &&
    left.x + left.width >= right.x &&
    left.y <= right.y + right.height &&
    left.y + left.height >= right.y
  );
}

function doesNumberAnnotationIntersectRect(annotation: NumberAnnotation, rect: SelectionRect) {
  return doRectsIntersect(resolveNumberAnnotationLayout(annotation).bounds, rect);
}

function doesTextAnnotationIntersectRect(annotation: TextAnnotation, rect: SelectionRect) {
  const layout = resolveTextAnnotationLayout(annotation);
  if (!doRectsIntersect(layout.bounds, rect)) {
    return false;
  }

  if (layout.corners.some((point) => isPointInRect(point, rect))) {
    return true;
  }

  const rectCorners = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height },
  ];
  if (rectCorners.some((point) => isPointInPolygon(point, layout.corners))) {
    return true;
  }

  const textEdges: Array<[Point, Point]> = layout.corners.map((point, index) => [point, layout.corners[(index + 1) % layout.corners.length]]);
  const rectEdges: Array<[Point, Point]> = [
    [rectCorners[0], rectCorners[1]],
    [rectCorners[1], rectCorners[2]],
    [rectCorners[2], rectCorners[3]],
    [rectCorners[3], rectCorners[0]],
  ];
  return textEdges.some(([start, end]) => rectEdges.some(([edgeStart, edgeEnd]) => doLineSegmentsIntersect(start, end, edgeStart, edgeEnd)));
}

function doesShapeAnnotationIntersectRect(annotation: ShapeAnnotation, rect: SelectionRect) {
  const bounds = resolveShapeAnnotationBounds(annotation);
  if (!doRectsIntersect(bounds, rect)) {
    return false;
  }

  if (annotation.kind === "rect" || annotation.kind === "ellipse") {
    return true;
  }

  if (isPointInRect(annotation.start, rect) || isPointInRect(annotation.end, rect)) {
    return true;
  }

  const corners = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height },
  ];
  const edges: Array<[Point, Point]> = [
    [corners[0], corners[1]],
    [corners[1], corners[2]],
    [corners[2], corners[3]],
    [corners[3], corners[0]],
  ];

  return edges.some(([start, end]) => doLineSegmentsIntersect(annotation.start, annotation.end, start, end));
}

function doesEffectAnnotationIntersectRect(annotation: EffectAnnotation, rect: SelectionRect) {
  return doRectsIntersect(resolveEffectAnnotationBounds(annotation), rect);
}

function doLineSegmentsIntersect(firstStart: Point, firstEnd: Point, secondStart: Point, secondEnd: Point) {
  const cross = (origin: Point, target: Point, point: Point) =>
    (target.x - origin.x) * (point.y - origin.y) - (target.y - origin.y) * (point.x - origin.x);
  const within = (value: number, left: number, right: number) =>
    value >= Math.min(left, right) - 0.0001 && value <= Math.max(left, right) + 0.0001;
  const onSegment = (start: Point, end: Point, point: Point) =>
    Math.abs(cross(start, end, point)) <= 0.0001 && within(point.x, start.x, end.x) && within(point.y, start.y, end.y);

  const firstCrossStart = cross(firstStart, firstEnd, secondStart);
  const firstCrossEnd = cross(firstStart, firstEnd, secondEnd);
  const secondCrossStart = cross(secondStart, secondEnd, firstStart);
  const secondCrossEnd = cross(secondStart, secondEnd, firstEnd);

  if (
    ((firstCrossStart > 0 && firstCrossEnd < 0) || (firstCrossStart < 0 && firstCrossEnd > 0)) &&
    ((secondCrossStart > 0 && secondCrossEnd < 0) || (secondCrossStart < 0 && secondCrossEnd > 0))
  ) {
    return true;
  }

  return (
    onSegment(firstStart, firstEnd, secondStart) ||
    onSegment(firstStart, firstEnd, secondEnd) ||
    onSegment(secondStart, secondEnd, firstStart) ||
    onSegment(secondStart, secondEnd, firstEnd)
  );
}

function isPointInPolygon(point: Point, polygon: Point[]) {
  let inside = false;
  for (let currentIndex = 0, previousIndex = polygon.length - 1; currentIndex < polygon.length; previousIndex = currentIndex, currentIndex += 1) {
    const current = polygon[currentIndex];
    const previous = polygon[previousIndex];
    const intersects =
      current.y > point.y !== previous.y > point.y &&
      point.x < ((previous.x - current.x) * (point.y - current.y)) / ((previous.y - current.y) || 0.000001) + current.x;
    if (intersects) {
      inside = !inside;
    }
  }
  return inside;
}

function doesPenAnnotationIntersectRect(annotation: PenAnnotation, rect: SelectionRect) {
  const bounds = resolvePenAnnotationBounds(annotation);
  if (!doRectsIntersect(bounds, rect)) {
    return false;
  }

  if (annotation.points.length === 0) {
    return false;
  }

  if (annotation.points.some((point) => isPointInRect(point, rect))) {
    return true;
  }

  if (annotation.points.length === 1) {
    return doRectsIntersect(bounds, rect);
  }

  const corners = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height },
  ];
  const edges: Array<[Point, Point]> = [
    [corners[0], corners[1]],
    [corners[1], corners[2]],
    [corners[2], corners[3]],
    [corners[3], corners[0]],
  ];

  for (let index = 1; index < annotation.points.length; index += 1) {
    const start = annotation.points[index - 1];
    const end = annotation.points[index];
    if (isPointInRect(start, rect) || isPointInRect(end, rect)) {
      return true;
    }
    if (edges.some(([edgeStart, edgeEnd]) => doLineSegmentsIntersect(start, end, edgeStart, edgeEnd))) {
      return true;
    }
  }

  return false;
}

function resolvePreferredObjectMarqueeFamily(
  selectedTextIds: string[],
  selectedShapeIds: string[],
  selectedPenIds: string[],
  selectedNumberIds: string[],
  selectedEffectIds: string[],
): ObjectSelectionFamily | null {
  const selectedFamilies = [
    selectedTextIds.length > 0 ? "text" : null,
    selectedShapeIds.length > 0 ? "shape" : null,
    selectedPenIds.length > 0 ? "pen" : null,
    selectedNumberIds.length > 0 ? "number" : null,
    selectedEffectIds.length > 0 ? "effect" : null,
  ].filter((family): family is ObjectSelectionFamily => family !== null);

  if (selectedFamilies.length !== 1) {
    return null;
  }

  return selectedFamilies[0];
}

function resolveObjectMarqueeSelection(
  annotations: Annotation[],
  rect: SelectionRect,
  preferredFamily: ObjectSelectionFamily | null,
): ObjectMarqueeResolution {
  const hitTextIds = annotations
    .filter((annotation): annotation is TextAnnotation => annotation.kind === "text" && doesTextAnnotationIntersectRect(annotation, rect))
    .map((annotation) => annotation.id);
  const hitShapeIds = annotations
    .filter(
      (annotation): annotation is ShapeAnnotation =>
        (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") &&
        doesShapeAnnotationIntersectRect(annotation, rect),
    )
    .map((annotation) => annotation.id);
  const hitPenIds = annotations
    .filter((annotation): annotation is PenAnnotation => annotation.kind === "pen" && doesPenAnnotationIntersectRect(annotation, rect))
    .map((annotation) => annotation.id);
  const hitNumberIds = annotations
    .filter((annotation): annotation is NumberAnnotation => annotation.kind === "number" && doesNumberAnnotationIntersectRect(annotation, rect))
    .map((annotation) => annotation.id);
  const hitEffectIds = annotations
    .filter((annotation): annotation is EffectAnnotation => annotation.kind === "effect" && doesEffectAnnotationIntersectRect(annotation, rect))
    .map((annotation) => annotation.id);
  const counts = {
    text: hitTextIds.length,
    shape: hitShapeIds.length,
    pen: hitPenIds.length,
    number: hitNumberIds.length,
    effect: hitEffectIds.length,
  };

  const topmostFamily = (() => {
    for (let index = annotations.length - 1; index >= 0; index -= 1) {
      const annotation = annotations[index];
      if (annotation.kind === "text" && hitTextIds.includes(annotation.id)) {
        return "text" as ObjectSelectionFamily;
      }
      if ((annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") && hitShapeIds.includes(annotation.id)) {
        return "shape" as ObjectSelectionFamily;
      }
      if (annotation.kind === "pen" && hitPenIds.includes(annotation.id)) {
        return "pen" as ObjectSelectionFamily;
      }
      if (annotation.kind === "number" && hitNumberIds.includes(annotation.id)) {
        return "number" as ObjectSelectionFamily;
      }
      if (annotation.kind === "effect" && hitEffectIds.includes(annotation.id)) {
        return "effect" as ObjectSelectionFamily;
      }
    }
    return null;
  })();

  const family =
    preferredFamily === "text" && hitTextIds.length > 0
      ? "text"
      : preferredFamily === "shape" && hitShapeIds.length > 0
      ? "shape"
      : preferredFamily === "pen" && hitPenIds.length > 0
      ? "pen"
      : preferredFamily === "number" && hitNumberIds.length > 0
        ? "number"
        : preferredFamily === "effect" && hitEffectIds.length > 0
          ? "effect"
          : topmostFamily;

  if (!family) {
    return { family: null, ids: [], primaryId: null, counts };
  }

  const ids =
    family === "text"
      ? hitTextIds
      : family === "shape"
        ? hitShapeIds
        : family === "pen"
          ? hitPenIds
          : family === "number"
            ? hitNumberIds
            : hitEffectIds;
  const primaryId =
    [...annotations]
      .reverse()
      .find((annotation) => {
        if (family === "shape") {
          return (
            (annotation.kind === "line" || annotation.kind === "rect" || annotation.kind === "ellipse" || annotation.kind === "arrow") &&
            ids.includes(annotation.id)
          );
        }
        return annotation.kind === family && ids.includes(annotation.id);
      })
      ?.id ?? null;

  return {
    family,
    ids,
    primaryId,
    counts,
  };
}

function resolveEffectTransformBounds(
  mode: EffectTransformMode,
  originBounds: SelectionRect,
  startPointer: Point,
  currentPointer: Point,
  selection: SelectionRect,
  minSize = 12,
) {
  const deltaX = currentPointer.x - startPointer.x;
  const deltaY = currentPointer.y - startPointer.y;

  if (mode === "move") {
    const maxX = selection.x + selection.width - originBounds.width;
    const maxY = selection.y + selection.height - originBounds.height;
    return {
      x: clamp(originBounds.x + deltaX, selection.x, Math.max(selection.x, maxX)),
      y: clamp(originBounds.y + deltaY, selection.y, Math.max(selection.y, maxY)),
      width: originBounds.width,
      height: originBounds.height,
    };
  }

  let left = originBounds.x;
  let top = originBounds.y;
  let right = originBounds.x + originBounds.width;
  let bottom = originBounds.y + originBounds.height;

  if (mode.includes("w")) {
    left = clamp(originBounds.x + deltaX, selection.x, right - minSize);
  }
  if (mode.includes("e")) {
    right = clamp(originBounds.x + originBounds.width + deltaX, left + minSize, selection.x + selection.width);
  }
  if (mode.includes("n")) {
    top = clamp(originBounds.y + deltaY, selection.y, bottom - minSize);
  }
  if (mode.includes("s")) {
    bottom = clamp(originBounds.y + originBounds.height + deltaY, top + minSize, selection.y + selection.height);
  }

  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}

function resolveTextAnnotationBounds(annotation: TextAnnotation) {
  return resolveTextAnnotationLayout(annotation).bounds;
}

function resolveObjectSelectionAnnotationBounds(annotation: ObjectSelectionAnnotation): SelectionRect {
  if (annotation.kind === "text") {
    return resolveTextAnnotationBounds(annotation);
  }
  if (annotation.kind === "pen") {
    return resolvePenAnnotationBounds(annotation);
  }
  if (annotation.kind === "number") {
    return resolveNumberAnnotationLayout(annotation).bounds;
  }
  if (annotation.kind === "effect") {
    return resolveEffectAnnotationBounds(annotation);
  }
  return resolveShapeAnnotationBounds(annotation);
}

function resolveObjectSelectionGroupBounds(annotations: ObjectSelectionAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolveObjectSelectionAnnotationBounds(annotation));
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function clampTextPointToSelection(point: Point, annotation: TextAnnotation, selection: SelectionRect): Point {
  const layout = resolveTextAnnotationLayout(annotation);
  const offsetX = layout.bounds.x - annotation.point.x;
  const offsetY = layout.bounds.y - annotation.point.y;
  const minX = selection.x - offsetX;
  const minY = selection.y - offsetY;
  const maxX = selection.x + selection.width - layout.bounds.width - offsetX;
  const maxY = selection.y + selection.height - layout.bounds.height - offsetY;

  return {
    x: clamp(point.x, minX, maxX),
    y: clamp(point.y, minY, maxY),
  };
}

function fitTextAnnotationToSelection(annotation: TextAnnotation, selection: SelectionRect) {
  const point = clampTextPointToSelection(annotation.point, annotation, selection);
  if (point.x === annotation.point.x && point.y === annotation.point.y) {
    return annotation;
  }
  return {
    ...annotation,
    point,
  };
}

function resolveTextGroupBounds(annotations: TextAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolveTextAnnotationBounds(annotation));
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function resolveShapeGroupBounds(annotations: ShapeAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolveShapeAnnotationBounds(annotation));
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function resolveNumberGroupBounds(annotations: NumberAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolveNumberAnnotationLayout(annotation).bounds);
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function resolvePenGroupBounds(annotations: PenAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolvePenAnnotationBounds(annotation));
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function resolveEffectGroupBounds(annotations: EffectAnnotation[]): SelectionRect {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  const bounds = annotations.map((annotation) => resolveEffectAnnotationBounds(annotation));
  const minX = Math.min(...bounds.map((entry) => entry.x));
  const minY = Math.min(...bounds.map((entry) => entry.y));
  const maxX = Math.max(...bounds.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...bounds.map((entry) => entry.y + entry.height));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function clampGroupDeltaToSelection(delta: Point, bounds: SelectionRect, selection: SelectionRect): Point {
  const minX = selection.x - bounds.x;
  const minY = selection.y - bounds.y;
  const maxX = selection.x + selection.width - (bounds.x + bounds.width);
  const maxY = selection.y + selection.height - (bounds.y + bounds.height);

  return {
    x: clamp(delta.x, minX, maxX),
    y: clamp(delta.y, minY, maxY),
  };
}

function resolvePasteOffset(requested: Point, groupBounds: SelectionRect, selection: SelectionRect) {
  const primary = clampGroupDeltaToSelection(requested, groupBounds, selection);
  if (Math.abs(primary.x) >= 1 || Math.abs(primary.y) >= 1) {
    return primary;
  }

  const fallback = clampGroupDeltaToSelection(
    {
      x: requested.x === 0 ? 24 : -requested.x,
      y: requested.y === 0 ? 24 : -requested.y,
    },
    groupBounds,
    selection,
  );

  if (Math.abs(fallback.x) >= 1 || Math.abs(fallback.y) >= 1) {
    return fallback;
  }

  return primary;
}

function resolveSnappedTextDrag(
  rawDelta: Point,
  groupBounds: SelectionRect,
  selection: SelectionRect,
  annotations: Annotation[],
  selectedIds: string[],
  threshold = 6,
) {
  let delta = clampGroupDeltaToSelection(rawDelta, groupBounds, selection);
  let movingBounds = offsetRect(groupBounds, delta);
  const otherBounds = annotations
    .filter((annotation): annotation is TextAnnotation => annotation.kind === "text" && !selectedIds.includes(annotation.id))
    .map((annotation) => resolveTextAnnotationBounds(annotation));

  const guides: SnapGuide[] = [];
  const verticalTargets = buildVerticalSnapTargets(selection, otherBounds);
  const verticalSnap = resolveAxisSnap("vertical", movingBounds, groupBounds, selection, delta, verticalTargets, threshold);
  if (verticalSnap) {
    delta = { ...delta, x: verticalSnap.delta };
    movingBounds = offsetRect(groupBounds, delta);
    guides.push(verticalSnap.guide);
  }

  const horizontalTargets = buildHorizontalSnapTargets(selection, otherBounds);
  const horizontalSnap = resolveAxisSnap("horizontal", movingBounds, groupBounds, selection, delta, horizontalTargets, threshold);
  if (horizontalSnap) {
    delta = { ...delta, y: horizontalSnap.delta };
    movingBounds = offsetRect(groupBounds, delta);
    guides.push(horizontalSnap.guide);
  }

  return {
    delta,
    guides,
  };
}

function buildVerticalSnapTargets(selection: SelectionRect, boundsList: SelectionRect[]): SnapGuide[] {
  const selectionTargets: SnapGuide[] = [
    {
      orientation: "vertical",
      position: selection.x,
      start: selection.y,
      end: selection.y + selection.height,
      source: "selection",
    },
    {
      orientation: "vertical",
      position: selection.x + selection.width / 2,
      start: selection.y,
      end: selection.y + selection.height,
      source: "selection",
    },
    {
      orientation: "vertical",
      position: selection.x + selection.width,
      start: selection.y,
      end: selection.y + selection.height,
      source: "selection",
    },
  ];

  const annotationTargets = boundsList.flatMap((bounds) => [
    {
      orientation: "vertical" as const,
      position: bounds.x,
      start: bounds.y,
      end: bounds.y + bounds.height,
      source: "annotation" as const,
    },
    {
      orientation: "vertical" as const,
      position: bounds.x + bounds.width / 2,
      start: bounds.y,
      end: bounds.y + bounds.height,
      source: "annotation" as const,
    },
    {
      orientation: "vertical" as const,
      position: bounds.x + bounds.width,
      start: bounds.y,
      end: bounds.y + bounds.height,
      source: "annotation" as const,
    },
  ]);

  return [...selectionTargets, ...annotationTargets];
}

function buildHorizontalSnapTargets(selection: SelectionRect, boundsList: SelectionRect[]): SnapGuide[] {
  const selectionTargets: SnapGuide[] = [
    {
      orientation: "horizontal",
      position: selection.y,
      start: selection.x,
      end: selection.x + selection.width,
      source: "selection",
    },
    {
      orientation: "horizontal",
      position: selection.y + selection.height / 2,
      start: selection.x,
      end: selection.x + selection.width,
      source: "selection",
    },
    {
      orientation: "horizontal",
      position: selection.y + selection.height,
      start: selection.x,
      end: selection.x + selection.width,
      source: "selection",
    },
  ];

  const annotationTargets = boundsList.flatMap((bounds) => [
    {
      orientation: "horizontal" as const,
      position: bounds.y,
      start: bounds.x,
      end: bounds.x + bounds.width,
      source: "annotation" as const,
    },
    {
      orientation: "horizontal" as const,
      position: bounds.y + bounds.height / 2,
      start: bounds.x,
      end: bounds.x + bounds.width,
      source: "annotation" as const,
    },
    {
      orientation: "horizontal" as const,
      position: bounds.y + bounds.height,
      start: bounds.x,
      end: bounds.x + bounds.width,
      source: "annotation" as const,
    },
  ]);

  return [...selectionTargets, ...annotationTargets];
}

function resolveAxisSnap(
  orientation: "vertical" | "horizontal",
  movingBounds: SelectionRect,
  groupBounds: SelectionRect,
  selection: SelectionRect,
  currentDelta: Point,
  targets: SnapGuide[],
  threshold: number,
) {
  const anchors =
    orientation === "vertical"
      ? [movingBounds.x, movingBounds.x + movingBounds.width / 2, movingBounds.x + movingBounds.width]
      : [movingBounds.y, movingBounds.y + movingBounds.height / 2, movingBounds.y + movingBounds.height];

  let bestMatch: { diff: number; guide: SnapGuide } | null = null;
  for (const anchor of anchors) {
    for (const target of targets) {
      const diff = target.position - anchor;
      if (Math.abs(diff) > threshold) {
        continue;
      }
      if (!bestMatch || Math.abs(diff) < Math.abs(bestMatch.diff)) {
        bestMatch = { diff, guide: target };
      }
    }
  }

  if (!bestMatch) {
    return null;
  }

  if (orientation === "vertical") {
    const proposedDelta = currentDelta.x + bestMatch.diff;
    const clamped = clampGroupDeltaToSelection({ x: proposedDelta, y: currentDelta.y }, groupBounds, selection).x;
    if (Math.abs(clamped - proposedDelta) > 0.01) {
      return null;
    }

    const snappedBounds = offsetRect(groupBounds, { x: clamped, y: currentDelta.y });
    return {
      delta: clamped,
      guide: {
        ...bestMatch.guide,
        start: Math.min(bestMatch.guide.start, snappedBounds.y),
        end: Math.max(bestMatch.guide.end, snappedBounds.y + snappedBounds.height),
      },
    };
  }

  const proposedDelta = currentDelta.y + bestMatch.diff;
  const clamped = clampGroupDeltaToSelection({ x: currentDelta.x, y: proposedDelta }, groupBounds, selection).y;
  if (Math.abs(clamped - proposedDelta) > 0.01) {
    return null;
  }

  const snappedBounds = offsetRect(groupBounds, { x: currentDelta.x, y: clamped });
  return {
    delta: clamped,
    guide: {
      ...bestMatch.guide,
      start: Math.min(bestMatch.guide.start, snappedBounds.x),
      end: Math.max(bestMatch.guide.end, snappedBounds.x + snappedBounds.width),
    },
  };
}

function distance(left: Point, right: Point) {
  const dx = left.x - right.x;
  const dy = left.y - right.y;
  return Math.sqrt(dx * dx + dy * dy);
}

function areShapeAnnotationsEqual(left: ShapeAnnotation, right: ShapeAnnotation) {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.color === right.color &&
    left.strokeWidth === right.strokeWidth &&
    arePointsEqual(left.start, right.start) &&
    arePointsEqual(left.end, right.end)
  );
}

function arePenAnnotationsEqual(left: PenAnnotation, right: PenAnnotation) {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.color === right.color &&
    left.strokeWidth === right.strokeWidth &&
    left.points.length === right.points.length &&
    left.points.every((point, index) => arePointsEqual(point, right.points[index]))
  );
}

function areTextAnnotationsEqual(left: TextAnnotation, right: TextAnnotation) {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.style === right.style &&
    left.color === right.color &&
    left.fontSize === right.fontSize &&
    left.rotation === right.rotation &&
    Math.abs(left.opacity - right.opacity) < 0.001 &&
    left.point.x === right.point.x &&
    left.point.y === right.point.y &&
    left.text === right.text
  );
}

function areEffectAnnotationsEqual(left: EffectAnnotation, right: EffectAnnotation) {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.effect === right.effect &&
    Math.abs(left.intensity - right.intensity) < 0.001 &&
    left.start.x === right.start.x &&
    left.start.y === right.start.y &&
    left.end.x === right.end.x &&
    left.end.y === right.end.y
  );
}

function areNumberAnnotationsEqual(left: NumberAnnotation, right: NumberAnnotation) {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.value === right.value &&
    left.color === right.color &&
    left.size === right.size &&
    arePointsEqual(left.point, right.point)
  );
}

function arePointsEqual(left: Point, right: Point) {
  return Math.abs(left.x - right.x) < 0.001 && Math.abs(left.y - right.y) < 0.001;
}

function areSelectionRectsEqual(
  left: SelectionRect | null | undefined,
  right: SelectionRect | null | undefined,
) {
  if (!left && !right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }
  return (
    Math.abs(left.x - right.x) < 0.001 &&
    Math.abs(left.y - right.y) < 0.001 &&
    Math.abs(left.width - right.width) < 0.001 &&
    Math.abs(left.height - right.height) < 0.001
  );
}

function expandRect(rect: SelectionRect, padding: number): SelectionRect {
  return {
    x: rect.x - padding,
    y: rect.y - padding,
    width: rect.width + padding * 2,
    height: rect.height + padding * 2,
  };
}

function rotatePoint(point: Point, center: Point, degrees: number): Point {
  const radians = (degrees * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const dx = point.x - center.x;
  const dy = point.y - center.y;
  return {
    x: center.x + dx * cos - dy * sin,
    y: center.y + dx * sin + dy * cos,
  };
}

function rotateRectCorners(rect: SelectionRect, center: Point, degrees: number) {
  const corners = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height },
  ];

  if (degrees === 0) {
    return corners;
  }

  return corners.map((point) => rotatePoint(point, center, degrees));
}

function boundsFromPoints(points: Point[]): SelectionRect {
  const minX = Math.min(...points.map((point) => point.x));
  const minY = Math.min(...points.map((point) => point.y));
  const maxX = Math.max(...points.map((point) => point.x));
  const maxY = Math.max(...points.map((point) => point.y));
  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

function offsetRect(rect: SelectionRect, delta: Point): SelectionRect {
  return {
    x: rect.x + delta.x,
    y: rect.y + delta.y,
    width: rect.width,
    height: rect.height,
  };
}

function createCanvasContext(width: number, height: number) {
  if (typeof document === "undefined") {
    return null;
  }

  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width));
  canvas.height = Math.max(1, Math.round(height));
  return canvas.getContext("2d");
}

function clampRectToCanvas(rect: SelectionRect, canvasWidth: number, canvasHeight: number) {
  if (canvasWidth <= 0 || canvasHeight <= 0) {
    return null;
  }

  const x = clampNumber(Math.floor(rect.x), 0, canvasWidth - 1);
  const y = clampNumber(Math.floor(rect.y), 0, canvasHeight - 1);
  const maxWidth = Math.max(1, canvasWidth - x);
  const maxHeight = Math.max(1, canvasHeight - y);
  const width = clampNumber(Math.ceil(rect.width), 1, maxWidth);
  const height = clampNumber(Math.ceil(rect.height), 1, maxHeight);

  return {
    x,
    y,
    width,
    height,
  };
}

function normalizeTextContent(value: string) {
  return value.replace(/\r\n/g, "\n").trim();
}

function splitTextLines(value: string) {
  return value.replace(/\r\n/g, "\n").split("\n");
}

function resizeTextEditor(textarea: HTMLTextAreaElement) {
  textarea.style.height = "0px";
  textarea.style.height = `${textarea.scrollHeight}px`;
}

function resolveTextEditorLayout(editor: TextEditorState, selection: SelectionRect) {
  const padding = 8;
  const layout = resolveTextLayout(editor.text || "输入文字", editor.fontSize, editor.style, editor.color, editor.point, editor.rotation);
  const metrics = layout.metrics;
  const innerPaddingX = Math.max(layout.style.paddingX, 4);
  const innerPaddingY = Math.max(layout.style.paddingY, 4);
  const maxWidth = Math.max(24, selection.width - padding * 2);
  const minWidth = Math.min(120, maxWidth);
  const width = clamp(metrics.width + innerPaddingX * 2 + 18, minWidth, maxWidth);
  const maxHeight = Math.max(24, selection.height - padding * 2);
  const minHeight = Math.min(Math.max(42, Math.round(editor.fontSize * 1.9)), maxHeight);
  const height = clamp(metrics.height + innerPaddingY * 2 + 8, minHeight, maxHeight);
  const left = clamp(
    editor.point.x,
    selection.x + padding,
    Math.max(selection.x + padding, selection.x + selection.width - width - padding),
  );
  const top = clamp(
    editor.point.y,
    selection.y + padding,
    Math.max(selection.y + padding, selection.y + selection.height - height - padding),
  );

  return {
    left,
    top,
    width,
    height,
    lineHeight: metrics.lineHeight,
    paddingX: innerPaddingX,
    paddingY: innerPaddingY,
  };
}

function resolveTextEditorVisual(editor: TextEditorState) {
  const style = resolveTextStyleSpec(editor.style, editor.color, editor.fontSize);
  return {
    textColor: style.textColor,
    caretColor: editor.color,
    textShadow: style.strokeColor ? buildOutlineTextShadow(style.strokeColor) : undefined,
    containerBackground: style.boxFill ? toRgba(style.boxFill, style.boxOpacity) : "rgba(0, 0, 0, 0.40)",
    containerBorder: style.boxFill ? toRgba(editor.color, 0.85) : "rgba(255, 255, 255, 0.25)",
  };
}

function resolveNumberAnnotationLayout(annotation: NumberAnnotation) {
  const label = `${annotation.value}`;
  const fontSize = Math.max(12, annotation.size * 0.72);
  const metrics = measureTextBlock(label, fontSize);
  const baseRadius = Math.max(14, annotation.size * 0.65);
  const paddingX = Math.max(6, annotation.size * 0.28);
  const paddingY = Math.max(4, annotation.size * 0.18);
  const radius = Math.max(baseRadius, Math.max(metrics.width + paddingX * 2, metrics.height + paddingY * 2) / 2);
  const contrastColor = pickReadableTextColor(annotation.color);

  return {
    label,
    fontSize,
    radius,
    fillColor: annotation.color,
    textColor: contrastColor,
    borderColor: toRgba(contrastColor === "#ffffff" ? "#ffffff" : "#111111", 0.28),
    borderWidth: Math.max(1, annotation.size * 0.06),
    bounds: {
      x: annotation.point.x - radius,
      y: annotation.point.y - radius,
      width: radius * 2,
      height: radius * 2,
    },
  };
}

function resolveTextAnnotationLayout(annotation: TextAnnotation) {
  return resolveTextLayout(annotation.text, annotation.fontSize, annotation.style, annotation.color, annotation.point, annotation.rotation);
}

function resolveTextLayout(text: string, fontSize: number, styleKind: TextStyleKind, color: string, point: Point, rotation = 0) {
  const metrics = measureTextBlock(text, fontSize);
  const style = resolveTextStyleSpec(styleKind, color, fontSize);
  const halfStroke = style.strokeColor ? style.strokeWidth / 2 : 0;
  const boxRect = style.boxFill
    ? {
        x: point.x - style.paddingX,
        y: point.y - style.paddingY,
        width: metrics.width + style.paddingX * 2,
        height: metrics.height + style.paddingY * 2,
      }
    : null;
  const frame = {
    x: point.x - style.paddingX - halfStroke,
    y: point.y - style.paddingY - halfStroke,
    width: metrics.width + style.paddingX * 2 + halfStroke * 2,
    height: metrics.height + style.paddingY * 2 + halfStroke * 2,
  };
  const center = {
    x: frame.x + frame.width / 2,
    y: frame.y + frame.height / 2,
  };
  const corners = rotateRectCorners(frame, center, rotation);
  const bounds = boundsFromPoints(corners);

  return {
    metrics,
    style,
    boxRect,
    frame,
    center,
    corners,
    bounds,
  };
}

function resolveTextStyleSpec(style: TextStyleKind, color: string, fontSize: number) {
  if (style === "outline") {
    return {
      textColor: "#ffffff",
      strokeColor: color,
      strokeWidth: Math.max(2, fontSize * 0.14),
      boxFill: null,
      boxOpacity: 0,
      paddingX: 2,
      paddingY: 2,
      radius: 0,
    };
  }

  if (style === "background") {
    return {
      textColor: pickReadableTextColor(color),
      strokeColor: null,
      strokeWidth: 0,
      boxFill: color,
      boxOpacity: 1,
      paddingX: Math.max(8, fontSize * 0.34),
      paddingY: Math.max(4, fontSize * 0.18),
      radius: Math.max(6, fontSize * 0.24),
    };
  }

  if (style === "highlight") {
    return {
      textColor: pickReadableTextColor(color),
      strokeColor: null,
      strokeWidth: 0,
      boxFill: color,
      boxOpacity: 0.32,
      paddingX: Math.max(6, fontSize * 0.26),
      paddingY: Math.max(2, fontSize * 0.08),
      radius: Math.max(4, fontSize * 0.16),
    };
  }

  return {
    textColor: color,
    strokeColor: null,
    strokeWidth: 0,
    boxFill: null,
    boxOpacity: 0,
    paddingX: 0,
    paddingY: 0,
    radius: 0,
  };
}

function buildOutlineTextShadow(color: string) {
  return [
    `1px 0 0 ${color}`,
    `-1px 0 0 ${color}`,
    `0 1px 0 ${color}`,
    `0 -1px 0 ${color}`,
    `1px 1px 0 ${color}`,
    `-1px 1px 0 ${color}`,
    `1px -1px 0 ${color}`,
    `-1px -1px 0 ${color}`,
  ].join(", ");
}

function drawRoundedRect(context: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number, fillStyle: string) {
  const safeRadius = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.moveTo(x + safeRadius, y);
  context.lineTo(x + width - safeRadius, y);
  context.quadraticCurveTo(x + width, y, x + width, y + safeRadius);
  context.lineTo(x + width, y + height - safeRadius);
  context.quadraticCurveTo(x + width, y + height, x + width - safeRadius, y + height);
  context.lineTo(x + safeRadius, y + height);
  context.quadraticCurveTo(x, y + height, x, y + height - safeRadius);
  context.lineTo(x, y + safeRadius);
  context.quadraticCurveTo(x, y, x + safeRadius, y);
  context.closePath();
  context.fillStyle = fillStyle;
  context.fill();
}

let textMeasureContext: CanvasRenderingContext2D | null | undefined;

function getTextMeasureContext() {
  if (textMeasureContext !== undefined) {
    return textMeasureContext;
  }

  if (typeof document === "undefined") {
    textMeasureContext = null;
    return textMeasureContext;
  }

  const canvas = document.createElement("canvas");
  textMeasureContext = canvas.getContext("2d");
  return textMeasureContext;
}

function measureTextBlock(text: string, fontSize: number): TextMetrics {
  const normalizedFontSize = Math.max(10, fontSize);
  const lineHeight = Math.max(normalizedFontSize * 1.35, 20);
  const lines = splitTextLines(text || " ");
  const context = getTextMeasureContext();

  if (context) {
    context.font = `600 ${normalizedFontSize}px "MiSans","Segoe UI","PingFang SC",sans-serif`;
    const width = Math.max(
      ...lines.map((line) => Math.ceil(context.measureText(line || " ").width)),
      Math.ceil(normalizedFontSize * 0.75),
    );
    return {
      width,
      height: lineHeight * lines.length,
      lineHeight,
    };
  }

  const fallbackWidth = Math.max(...lines.map((line) => line.length), 1) * normalizedFontSize * 0.62;
  return {
    width: fallbackWidth,
    height: lineHeight * lines.length,
    lineHeight,
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function clampNumber(value: number, min: number, max: number) {
  if (Number.isNaN(value)) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}

function pickReadableTextColor(hex: string) {
  const normalized = hex.replace("#", "");
  const value = normalized.length === 3 ? normalized.split("").map((part) => `${part}${part}`).join("") : normalized;
  const red = Number.parseInt(value.slice(0, 2), 16);
  const green = Number.parseInt(value.slice(2, 4), 16);
  const blue = Number.parseInt(value.slice(4, 6), 16);
  const brightness = (red * 299 + green * 587 + blue * 114) / 1000;
  return brightness >= 150 ? "#111111" : "#ffffff";
}

function toRgba(hex: string, alpha: number) {
  const normalized = hex.replace("#", "");
  const value = normalized.length === 3 ? normalized.split("").map((part) => `${part}${part}`).join("") : normalized;
  const red = Number.parseInt(value.slice(0, 2), 16);
  const green = Number.parseInt(value.slice(2, 4), 16);
  const blue = Number.parseInt(value.slice(4, 6), 16);
  return `rgba(${red}, ${green}, ${blue}, ${clamp(alpha, 0, 1)})`;
}

function loadImage(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`IMAGE_LOAD_FAILED:${src}`));
    image.src = src;
  });
}

async function writeTauriPipelineLog(level: "info" | "error", message: string) {
  if (!hasDesktopRuntime()) {
    return;
  }

  try {
    tauriLogModulePromise ??= import("@tauri-apps/plugin-log");
    const logger = await tauriLogModulePromise;
    if (level === "error") {
      await logger.error(message);
      return;
    }
    await logger.info(message);
  } catch {
    // Logging must never block the screenshot pipeline.
  }
}

function emitPipelineInfo(message: string) {
  console.info(message);
  void writeTauriPipelineLog("info", message);
}

function emitPipelineError(message: string, error: unknown) {
  console.error(message, error);
  const detail = error instanceof Error ? error.message : String(error);
  void writeTauriPipelineLog("error", `${message}: ${detail}`);
}

function buildPreviewProtocolUrl(path: string) {
  const encoded = encodeUrlSafeBase64Utf8(path);
  const windowsStyleUrl = `http://bexo-preview.localhost/${encoded}`;
  const genericUrl = `bexo-preview://localhost/${encoded}`;
  return navigator.userAgent.includes("Windows") ? windowsStyleUrl : genericUrl;
}

function encodeUrlSafeBase64Utf8(value: string) {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  const chunkSize = 0x8000;

  for (let index = 0; index < bytes.length; index += chunkSize) {
    const chunk = bytes.subarray(index, index + chunkSize);
    binary += String.fromCharCode(...chunk);
  }

  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}

function buildToolHotkeyMap(): ReadonlyMap<string, ToolKind> {
  const map = new Map<string, ToolKind>();

  for (const [shortcut, tool] of FIXED_SCREENSHOT_TOOL_HOTKEYS) {
    const normalized = normalizeOverlayShortcut(shortcut);
    if (!normalized) {
      continue;
    }
    map.set(normalized, tool);
  }

  return map;
}

function resolveToolHotkeyFromKeyboardEvent(
  event: KeyboardEvent,
  toolHotkeyMap: ReadonlyMap<string, ToolKind>,
): ToolKind | null {
  const normalized = normalizeOverlayKeyboardEvent(event);
  if (!normalized) {
    return null;
  }

  return toolHotkeyMap.get(normalized) ?? null;
}

function normalizeOverlayShortcut(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const modifiers = new Set<string>();
  let key: string | null = null;

  for (const part of trimmed.split("+")) {
    const token = normalizeOverlayShortcutToken(part);
    if (!token) {
      return null;
    }

    if (isOverlayModifierToken(token)) {
      modifiers.add(token);
      continue;
    }

    if (key) {
      return null;
    }
    key = token;
  }

  if (!key && modifiers.size === 0) {
    return null;
  }

  return formatOverlayHotkey(modifiers, key);
}

function normalizeOverlayKeyboardEvent(event: KeyboardEvent): string | null {
  const modifiers = new Set<string>();
  if (event.ctrlKey) {
    modifiers.add("ctrl");
  }
  if (event.altKey) {
    modifiers.add("alt");
  }
  if (event.shiftKey) {
    modifiers.add("shift");
  }
  if (event.metaKey) {
    modifiers.add("super");
  }

  const keyToken = normalizeOverlayKeyboardKey(event.key);
  if (!keyToken) {
    return null;
  }

  if (isOverlayModifierToken(keyToken)) {
    modifiers.add(keyToken);
    return formatOverlayHotkey(modifiers, null);
  }

  return formatOverlayHotkey(modifiers, keyToken);
}

function formatOverlayHotkey(modifiers: ReadonlySet<string>, key: string | null): string {
  const parts: string[] = [];
  for (const token of ["ctrl", "alt", "shift", "super"]) {
    if (modifiers.has(token)) {
      parts.push(token);
    }
  }
  if (key) {
    parts.push(key);
  }
  return parts.join("+");
}

function normalizeOverlayKeyboardKey(input: string): string | null {
  const normalized = input.trim().toLowerCase();
  if (!normalized) {
    return null;
  }

  if (/^[a-z0-9]$/.test(normalized)) {
    return normalized;
  }

  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(normalized)) {
    return normalized;
  }

  switch (normalized) {
    case "control":
    case "ctrl":
      return "ctrl";
    case "alt":
    case "altgraph":
      return "alt";
    case "shift":
      return "shift";
    case "meta":
    case "super":
    case "os":
      return "super";
    case " ":
    case "spacebar":
      return "space";
    case "tab":
      return "tab";
    case "enter":
    case "return":
      return "enter";
    case "backspace":
      return "backspace";
    case "delete":
    case "del":
      return "delete";
    case "escape":
    case "esc":
      return "escape";
    case "arrowup":
    case "arrowdown":
    case "arrowleft":
    case "arrowright":
      return normalized;
    default:
      return null;
  }
}

function normalizeOverlayShortcutToken(input: string): string | null {
  const normalized = input.trim().toLowerCase();
  if (!normalized) {
    return null;
  }

  if (/^[a-z0-9]$/.test(normalized)) {
    return normalized;
  }

  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(normalized)) {
    return normalized;
  }

  switch (normalized) {
    case "ctrl":
    case "control":
    case "lctrl":
    case "leftctrl":
    case "leftcontrol":
    case "rctrl":
    case "rightctrl":
    case "rightcontrol":
      return "ctrl";
    case "alt":
    case "lalt":
    case "leftalt":
    case "ralt":
    case "rightalt":
    case "altgraph":
      return "alt";
    case "shift":
    case "lshift":
    case "leftshift":
    case "rshift":
    case "rightshift":
      return "shift";
    case "super":
    case "meta":
    case "win":
    case "windows":
    case "lwin":
    case "leftwin":
    case "leftwindows":
    case "rwin":
    case "rightwin":
    case "rightwindows":
      return "super";
    case "space":
      return "space";
    case "tab":
      return "tab";
    case "enter":
    case "return":
      return "enter";
    case "backspace":
      return "backspace";
    case "delete":
    case "del":
      return "delete";
    case "escape":
    case "esc":
      return "escape";
    case "arrowup":
    case "arrowdown":
    case "arrowleft":
    case "arrowright":
      return normalized;
    default:
      return null;
  }
}

function isOverlayModifierToken(token: string) {
  return token === "ctrl" || token === "alt" || token === "shift" || token === "super";
}

function formatNowForFileName() {
  const now = new Date();
  const year = now.getFullYear();
  const month = `${now.getMonth() + 1}`.padStart(2, "0");
  const day = `${now.getDate()}`.padStart(2, "0");
  const hours = `${now.getHours()}`.padStart(2, "0");
  const minutes = `${now.getMinutes()}`.padStart(2, "0");
  const seconds = `${now.getSeconds()}`.padStart(2, "0");
  return `${year}${month}${day}-${hours}${minutes}${seconds}`;
}

function areNativeInteractionStatesEqual(
  left: NativeInteractionStateView | null,
  right: NativeInteractionStateView | null,
) {
  if (!left && !right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }
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

function buildNativeInteractionRuntimeRequestKey(
  request: NativeInteractionRuntimeRequest,
) {
  return JSON.stringify({
    sessionId: request.sessionId,
    visible: request.visible,
    mode: request.mode,
    exclusionRects: request.exclusionRects.map((rect) => ({
      x: roundRuntimeGeometry(rect.x),
      y: roundRuntimeGeometry(rect.y),
      width: roundRuntimeGeometry(rect.width),
      height: roundRuntimeGeometry(rect.height),
    })),
    selection: request.selection
      ? {
          x: roundRuntimeGeometry(request.selection.x),
          y: roundRuntimeGeometry(request.selection.y),
          width: roundRuntimeGeometry(request.selection.width),
          height: roundRuntimeGeometry(request.selection.height),
        }
      : null,
    activeShape: request.activeShape
      ? {
          id: request.activeShape.id,
          kind: request.activeShape.kind,
          color: request.activeShape.color,
          strokeWidth: roundRuntimeGeometry(request.activeShape.strokeWidth),
          start: {
            x: roundRuntimeGeometry(request.activeShape.start.x),
            y: roundRuntimeGeometry(request.activeShape.start.y),
          },
          end: {
            x: roundRuntimeGeometry(request.activeShape.end.x),
            y: roundRuntimeGeometry(request.activeShape.end.y),
          },
        }
      : null,
    shapeCandidates: request.shapeCandidates.map((shape) => ({
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
    })),
    color: request.color,
    strokeWidth: roundRuntimeGeometry(request.strokeWidth),
  });
}

function roundRuntimeGeometry(value: number) {
  return Number(value.toFixed(3));
}

function areNativeEditableShapesEqual(
  left: NativeInteractionEditableShape | null,
  right: NativeInteractionEditableShape | null,
) {
  if (!left && !right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.color === right.color &&
    left.strokeWidth === right.strokeWidth &&
    arePointsEqual(left.start, right.start) &&
    arePointsEqual(left.end, right.end)
  );
}
