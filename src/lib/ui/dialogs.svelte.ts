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
}

export interface DialogSpec {
  title: string;
  /** Explanatory text, under the title. */
  body?: string;
  /** Warning, shown in a box. */
  warning?: string;
  fields?: Field[];
  /** The label of the button that confirms: a verb and its object, never "OK" (guidelines, 9.1). */
  confirmLabel: string;
  cancelLabel?: string;
  /** Destructive action: the confirmation button turns red. */
  danger?: boolean;
  /** Secondary action, between "Cancel" and the main button: the dialog closes, then the action runs. */
  extra?: { label: string; run: () => void };
}

export type Answers = Record<string, FieldValue>;

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
