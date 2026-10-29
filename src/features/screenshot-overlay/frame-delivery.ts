/** Latest-only visual state delivery; terminal input bypasses the animation-frame wait. */
export class FrameDelivery<T> {
  private frame: number | null = null;
  private pending: T | undefined;
  private disposed = false;
  private deliver: (value: T) => void;
  private request: (callback: () => void) => number;
  private cancel: (frame: number) => void;
  constructor(deliver: (value: T) => void,
    request: (callback: () => void) => number,
    cancel: (frame: number) => void) {
    this.deliver = deliver;
    this.request = request;
    this.cancel = cancel;
  }

  push(value: T, terminal = false) {
    if (this.disposed) return;
    this.pending = value;
    if (terminal) { this.flush(); return; }
    if (this.frame === null) this.frame = this.request(() => { this.frame = null; this.flush(); });
  }
  private flush() {
    if (this.frame !== null) this.cancel(this.frame);
    this.frame = null;
    const value = this.pending;
    this.pending = undefined;
    if (!this.disposed && value !== undefined) this.deliver(value);
  }
  dispose() {
    this.disposed = true;
    if (this.frame !== null) this.cancel(this.frame);
    this.frame = null;
    this.pending = undefined;
  }
}
