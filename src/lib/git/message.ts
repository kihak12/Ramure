/**
 * How the description of a commit is shown in the detail panel (guidelines, 9.27): its paragraphs
 * joined to the width of the panel, and the trailers of git set apart. The message itself is not
 * changed: "Reword" edits it as written.
 */

export interface Paragraph {
  text: string;
  /** Kept as written: a list, a quote, an indented block. */
  pre: boolean;
}

export interface MessageView {
  paragraphs: Paragraph[];
  /** The trailers of git ("Co-Authored-By: …"), one per line. */
  trailers: string[];
}

/** A trailer: a token, a colon, a space, then a value. */
const TRAILER = /^[A-Za-z][A-Za-z0-9-]*: \S/;
/** A line that keeps its paragraph as written: indented, a list sign, a quote, a heading. */
const KEEP = /^(\s|[-*•>#]|\d+[.)]\s)/;

/** Reads the description of a commit: the message without its subject line. */
export function readMessage(description: string): MessageView {
  const blocks = description
    .replace(/\r\n/g, '\n')
    .split(/\n[ \t]*\n/)
    .map((block) => block.replace(/^\n+|\n+$/g, ''))
    .filter((block) => block.trim() !== '');

  let trailers: string[] = [];
  const last = blocks.at(-1)?.split('\n');
  if (last && last.every((line) => TRAILER.test(line))) {
    trailers = last;
    blocks.pop();
  }

  const paragraphs = blocks.map((block) => {
    const lines = block.split('\n');
    const pre = lines.some((line) => KEEP.test(line));
    return { text: pre ? block : lines.map((line) => line.trim()).join(' '), pre };
  });
  return { paragraphs, trailers };
}
