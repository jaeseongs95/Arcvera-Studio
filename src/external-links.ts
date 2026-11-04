import { invoke } from '@tauri-apps/api/core';
import type { ILink, Terminal } from '@xterm/xterm';

const MAX_URL_LENGTH = 8192;

/** The host validates again; URL parsing must never silently remove controls. */
export function validateExternalUrl(value: string): string {
  if (!value || value.length > MAX_URL_LENGTH || /[\s\u0000-\u001f\u007f-\u009f\\]/u.test(value) || /%(?:0[0-9a-f]|1[0-9a-f]|7f)|%c2%[89][0-9a-f]/i.test(value) || !/^https?:\/\/[^/?#]+/i.test(value)) throw new Error('HTTP(S) 웹 주소만 열 수 있습니다.');
  let parsed: URL;
  try { parsed = new URL(value); } catch { throw new Error('올바른 웹 주소가 아닙니다.'); }
  if (!['http:', 'https:'].includes(parsed.protocol) || !parsed.hostname) throw new Error('HTTP(S) 웹 주소만 열 수 있습니다.');
  return parsed.href;
}

export async function openExternalUrl(url: string): Promise<void> {
  await invoke('open_external_url', { url: validateExternalUrl(url) });
}

function linksIn(text: string) {
  const links: { index: number; text: string }[] = [];
  const pattern = /https?:\/\/[^\s\u0000-\u001f\u007f-\u009f<>"'`\\]+/giu;
  for (const match of text.matchAll(pattern)) {
    // Sentence punctuation and unmatched enclosing brackets are not part of a URL.
    let value = match[0].replace(/[.,!?:;]+$/u, '');
    for (;;) {
      const last = value.at(-1); const opening = last === ')' ? '(' : last === ']' ? '[' : last === '}' ? '{' : '';
      if (!opening || value.split(last!).length <= value.split(opening).length) break;
      value = value.slice(0, -1).replace(/[.,!?:;]+$/u, '');
    }
    try { validateExternalUrl(value); links.push({ index: match.index!, text: value }); } catch { /* Invalid text remains text. */ }
  }
  return links;
}

/** Adds text nodes and safe anchors; no content is interpreted as HTML. */
export function appendLinkifiedText(parent: HTMLElement, text: string, onError: (error: unknown) => void = error => { console.error('외부 링크 열기 실패', error); }) {
  let offset = 0;
  for (const link of linksIn(text)) {
    parent.append(document.createTextNode(text.slice(offset, link.index)));
    const anchor = document.createElement('a');
    anchor.textContent = link.text; anchor.href = validateExternalUrl(link.text); anchor.title = anchor.href; anchor.rel = 'noopener noreferrer';
    anchor.addEventListener('click', event => {
      // A link inside a question's radio label must not select or submit it.
      event.preventDefault(); event.stopPropagation();
      void openExternalUrl(link.text).catch(onError);
    });
    parent.append(anchor); offset = link.index + link.text.length;
  }
  parent.append(document.createTextNode(text.slice(offset)));
}

/** Owns the plain URL provider and OSC 8 handler for one terminal's lifetime. */
export function installTerminalExternalLinks(terminal: Terminal, onError: (error: unknown) => void) {
  let disposed = false;
  const previous = terminal.options.linkHandler;
  const previousTitle = terminal.element?.getAttribute('title');
  const activate = (event: MouseEvent, url: string) => {
    event.preventDefault(); event.stopPropagation();
    if (!disposed) void openExternalUrl(url).catch(onError);
  };
  const hover = (_event: MouseEvent, url: string) => { if (!disposed) terminal.element?.setAttribute('title', url); };
  const leave = () => { if (previousTitle == null) terminal.element?.removeAttribute('title'); else terminal.element?.setAttribute('title', previousTitle); };
  const handler = { activate, hover, leave };
  terminal.options.linkHandler = handler;
  const provider = terminal.registerLinkProvider({
    provideLinks(y, callback) {
      if (disposed) { callback(undefined); return; }
      const buffer = terminal.buffer.active;
      let first = y - 1; let last = first; let count = 0;
      // Bounded lookup: at most one maximum URL's context on either side.
      while (first > 0 && buffer.getLine(first)?.isWrapped && count < MAX_URL_LENGTH) { first--; count += buffer.getLine(first)?.translateToString(true).length ?? 0; }
      count = 0;
      while (last + 1 < buffer.length && buffer.getLine(last + 1)?.isWrapped && count < MAX_URL_LENGTH) { last++; count += buffer.getLine(last)?.translateToString(true).length ?? 0; }
      let text = '';
      const starts: { x: number; y: number }[] = []; const ends: { x: number; y: number }[] = [];
      for (let row = first; row <= last; row++) {
        const line = buffer.getLine(row); if (!line) continue;
        const length = line.translateToString(true).length;
        let used = 0;
        for (let column = 0; column < line.length; column++) {
          const cell = line.getCell(column); if (!cell || !cell.getWidth()) continue;
          const chars = cell.getChars();
          // A wide character can leave an empty padding cell before wrapping.
          if (!chars && column === line.length - 1 && buffer.getLine(row + 1)?.isWrapped && buffer.getLine(row + 1)?.getCell(0)?.getWidth() === 2) continue;
          if (row === last && used >= length) break;
          const value = chars || ' ';
          for (let unit = 0; unit < value.length; unit++) { starts.push({ x: column + 1, y: row + 1 }); ends.push({ x: column + cell.getWidth(), y: row + 1 }); }
          text += value; used += value.length;
        }
      }
      const links: ILink[] = linksIn(text).flatMap(link => {
        const start = starts[link.index]; const end = ends[link.index + link.text.length - 1];
        return start && end && start.y <= y && end.y >= y ? [{ text: link.text, range: { start, end }, activate, hover, leave }] : [];
      });
      callback(links);
    },
  });
  return { dispose() { if (disposed) return; disposed = true; provider.dispose(); leave(); if (terminal.options.linkHandler === handler) terminal.options.linkHandler = previous; } };
}
