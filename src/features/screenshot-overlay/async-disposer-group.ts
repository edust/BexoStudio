export type AsyncDisposer = () => void;
export type AsyncDisposerRegistration = () => Promise<AsyncDisposer>;

export class AsyncDisposerGroup {
  readonly #disposers: AsyncDisposer[] = [];
  readonly #onCleanupError: (error: unknown) => void;
  #disposed = false;

  constructor(onCleanupError: (error: unknown) => void = () => undefined) {
    this.#onCleanupError = onCleanupError;
  }

  async attach(registration: AsyncDisposerRegistration): Promise<boolean> {
    if (this.#disposed) {
      return false;
    }

    const disposer = await registration();
    if (this.#disposed) {
      this.#safeDispose(disposer);
      return false;
    }

    this.#disposers.push(disposer);
    return true;
  }

  dispose(): void {
    this.#disposed = true;
    while (this.#disposers.length) {
      const disposer = this.#disposers.pop();
      if (disposer) {
        this.#safeDispose(disposer);
      }
    }
  }

  #safeDispose(disposer: AsyncDisposer): void {
    try {
      disposer();
    } catch (error) {
      this.#onCleanupError(error);
    }
  }
}
