// What the application announces to screen readers, from two regions kept in the page: a region
// created already filled is often not read, one that is filled is (guidelines, 11).

class Announcer {
  polite = $state('');
  urgent = $state('');
  // What is to be said on the next frame: two news of the same instant are said one after the
  // other, not the last one alone.
  #next = { polite: [] as string[], urgent: [] as string[] };

  /** Says `text`: as news, or at once for an error. The same text said again is said again. */
  say(text: string, urgentToo = false) {
    if (!text) return;
    const kind = urgentToo ? 'urgent' : 'polite';
    const queue = this.#next[kind];
    if (!queue.includes(text)) queue.push(text);
    if (queue.length > 1) return;
    // Emptied, then filled on the next frame: a reader notices the change.
    this[kind] = '';
    requestAnimationFrame(() => {
      this[kind] = queue.join(' ');
      this.#next[kind] = [];
    });
  }
}

export const announcer = new Announcer();
