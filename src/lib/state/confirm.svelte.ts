// Imperative confirm dialog, backed by a single <ConfirmHost /> mounted near
// the app root. Replaces the native, synchronous window.confirm():
//
//   if (!(await confirmDialog({ title: "Remove?", destructive: true }))) return;
//
// confirmDialog() returns a Promise<boolean> that resolves true when the user
// confirms and false on cancel / escape / overlay-dismiss.

export interface ConfirmOptions {
  title: string;
  description?: string;
  confirmText?: string;
  cancelText?: string;
  /** Style the confirm action as destructive (used for remove/teardown). */
  destructive?: boolean;
}

interface ConfirmRequest extends ConfirmOptions {
  resolve: (value: boolean) => void;
}

class ConfirmStore {
  current = $state<ConfirmRequest | null>(null);
  open = $state(false);

  request(opts: ConfirmOptions): Promise<boolean> {
    // Resolve any already-open request as cancelled before opening a new one.
    this.current?.resolve(false);
    return new Promise<boolean>((resolve) => {
      this.current = { ...opts, resolve };
      this.open = true;
    });
  }

  private settle(value: boolean) {
    const cur = this.current;
    this.current = null;
    this.open = false;
    cur?.resolve(value);
  }

  accept() {
    this.settle(true);
  }
  cancel() {
    this.settle(false);
  }
}

export const confirmStore = new ConfirmStore();

export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  return confirmStore.request(opts);
}
