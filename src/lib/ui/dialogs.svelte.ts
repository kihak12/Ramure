// Dialogs of the application: confirmations and small forms.
// `dialogs.ask(...)` returns a promise, resolved with the values entered, or `null` on cancel.

export type FieldValue = string | boolean | number;

export interface Field {
  name: string;
  label: string;
  type: 'text' | 'textarea' | 'select' | 'checkbox' | 'number';
  value?: FieldValue;
  options?: { value: string | number; label: string }[];
  placeholder?: string;
  hint?: string;
  required?: boolean;
  min?: number;
  max?: number;
  mono?: boolean;
  /** A text field that takes a file of this machine: a "Browse…" button beside it opens the
   *  picker of the system, under this title (guidelines, 9.10). */
  browse?: string;
}

export interface DialogSpec {
  title: string;
  /** Explanatory text, under the title; one line per thing it says. */
  body?: string;
  /** Warning, shown in a box; one line per thing to warn of. */
  warning?: string;
  /** A last word under the warning, in muted text: what happens next. */
  note?: string;
  fields?: Field[];
  /** The label of the button that confirms: a verb and its object, never "OK" (guidelines, 9.1). */
  confirmLabel: string;
  cancelLabel?: string;
  /** Destructive action: the confirmation button turns red. */
  danger?: boolean;
  /** A destructive action whose button takes the focus, so that Enter confirms: the delete of the
   *  Files module alone (guidelines, 9.10). */
  confirmFirst?: boolean;
  /** Secondary actions, between "Cancel" and the main button, in this order: the dialog closes,
   *  then the action runs. Four buttons at most on the last line (guidelines, 9.1). */
  extra?: Extra | Extra[];
  /**
   * Runs the action when the main button is pressed, before the dialog closes: the reason it
   * failed is said under the first field, and the dialog stays open with what was typed
   * (guidelines, 9.10 and 9.11). Null: done, the dialog closes.
   */
  check?: (answers: Answers) => Promise<string | null>;
  /** The label of the main button while `check` runs: "Renaming…" (guidelines, 9.1). */
  busyLabel?: string;
  /** The part of the first field selected when the dialog opens, as [start, end]: a name up to
   *  its extension. The whole text otherwise. */
  select?: [number, number];
}

export type Answers = Record<string, FieldValue>;

/** A secondary action of a dialog. */
export interface Extra {
  label: string;
  run: () => void;
}

class Dialogs {
  current = $state.raw<{ spec: DialogSpec; resolve: (answers: Answers | null) => void } | null>(null);

  ask(spec: DialogSpec): Promise<Answers | null> {
    // One dialog at a time: the previous one is canceled.
    this.current?.resolve(null);
    return new Promise((resolve) => {
      this.current = { spec, resolve };
    });
  }

  async confirm(spec: DialogSpec): Promise<boolean> {
    return (await this.ask(spec)) !== null;
  }

  close(answers: Answers | null) {
    const open = this.current;
    this.current = null;
    open?.resolve(answers);
  }
}

export const dialogs = new Dialogs();
