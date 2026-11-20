import { WidgetType } from '@codemirror/view';

/** A blank row that keeps the three columns of the conflict editor aligned (guidelines, 9.26). */
export class Spacer extends WidgetType {
  readonly height: number;
  constructor(height: number) {
    super();
    this.height = height;
  }
  eq(other: Spacer) {
    return other.height === this.height;
  }
  toDOM() {
    const blank = document.createElement('div');
    blank.className = 'cm-align';
    blank.style.height = `${this.height}px`;
    blank.setAttribute('aria-hidden', 'true');
    return blank;
  }
  get estimatedHeight() {
    return this.height;
  }
}
