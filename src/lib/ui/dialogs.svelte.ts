// Boîtes de dialogue de l'application : confirmations et petits formulaires.
// `dialogs.ask(...)` renvoie une promesse, résolue avec les valeurs saisies ou `null` si on annule.

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
  /** Texte d'explication, sous le titre. */
  body?: string;
  /** Mise en garde, affichée dans un encadré. */
  warning?: string;
  fields?: Field[];
  confirmLabel?: string;
  cancelLabel?: string;
  /** Action destructrice : le bouton de confirmation passe en rouge. */
  danger?: boolean;
}

export type Answers = Record<string, FieldValue>;

class Dialogs {
  current = $state.raw<{ spec: DialogSpec; resolve: (answers: Answers | null) => void } | null>(null);

  ask(spec: DialogSpec): Promise<Answers | null> {
    // Une seule boîte à la fois : la précédente est annulée.
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
