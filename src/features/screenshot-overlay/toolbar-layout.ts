type Rect = { x: number; y: number; width: number; height: number };
type Monitor = { relativeX: number; relativeY: number; displayWidth: number; displayHeight: number };

/** Keep both toolbar rows on the monitor containing the selection's centre. */
export function resolveScreenshotToolbarLayout(
  selection: Rect | null,
  monitors: Monitor[],
  viewport: { width: number; height: number },
  measured: { width: number; height: number },
) {
  const centre = { x: selection ? selection.x + selection.width / 2 : viewport.width / 2,
    y: selection ? selection.y + selection.height / 2 : viewport.height / 2 };
  const validMonitors = monitors.filter((monitor) => monitor.displayWidth > 0 && monitor.displayHeight > 0);
  const containing = validMonitors.find((monitor) => centre.x >= monitor.relativeX &&
    centre.x <= monitor.relativeX + monitor.displayWidth && centre.y >= monitor.relativeY &&
    centre.y <= monitor.relativeY + monitor.displayHeight);
  // A selection may straddle a gap between monitors: use the nearest screen, not the gap.
  const distance = (monitor: Monitor) => {
    const dx = centre.x - clamp(centre.x, monitor.relativeX, monitor.relativeX + monitor.displayWidth);
    const dy = centre.y - clamp(centre.y, monitor.relativeY, monitor.relativeY + monitor.displayHeight);
    return dx * dx + dy * dy;
  };
  const monitor = containing ?? [...validMonitors].sort((a, b) => distance(a) - distance(b))[0] ??
    { relativeX: 0, relativeY: 0, displayWidth: viewport.width, displayHeight: viewport.height };
  const margin = 8;
  const width = Math.min(measured.width, Math.max(1, monitor.displayWidth - margin * 2));
  const height = Math.min(measured.height, Math.max(1, monitor.displayHeight - margin * 2));
  const left = clamp(centre.x, monitor.relativeX + margin + width / 2,
    monitor.relativeX + monitor.displayWidth - margin - width / 2);
  const below = selection ? selection.y + selection.height + 12 : monitor.relativeY + margin;
  const above = selection ? selection.y - height - 12 : below;
  const bottom = monitor.relativeY + monitor.displayHeight - margin - height;
  const top = clamp(below <= bottom ? below : above, monitor.relativeY + margin, bottom);
  return { left, top, maxWidth: Math.max(1, monitor.displayWidth - margin * 2),
    maxHeight: Math.max(1, monitor.displayHeight - margin * 2) };
}

function clamp(value: number, min: number, max: number) { return Math.max(min, Math.min(max, value)); }
