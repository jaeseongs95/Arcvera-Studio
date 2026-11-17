// One document-lifetime tracker shared by all Desktop input handlers.
// Weak targets do not retain disposed conversations or terminal textareas.
const composingInputs = new WeakSet<EventTarget>();
let composingEnters = new WeakSet<EventTarget>();
document.addEventListener('compositionstart', event => { if (event.target) composingInputs.add(event.target); }, true);
document.addEventListener('compositionend', event => { if (event.target) composingInputs.delete(event.target); }, true);
document.addEventListener('focusout', event => { if (event.target) resetComposition(event.target); }, true);
const isEnter = (event: KeyboardEvent) => event.key === 'Enter' || event.code === 'Enter' || event.code === 'NumpadEnter';
document.addEventListener('keydown', event => {
  if (!event.target) return;
  if (!isEnter(event)) { composingEnters.delete(event.target); return; }
  // Windows can emit Process/229, compositionend, then Enter/13 for the
  // same physical press. Keep that press guarded until release, not a timer.
  if (event.isComposing || event.keyCode === 229 || isComposingTarget(event.target)) {
    composingEnters.add(event.target);
  // Leave the IME's own events alone; suppress only the trailing browser action.
  } else if (composingEnters.has(event.target)) event.preventDefault();
}, true);
document.addEventListener('keyup', event => { if (event.target && isEnter(event)) composingEnters.delete(event.target); }, true);
window.addEventListener('blur', () => { composingEnters = new WeakSet<EventTarget>(); });

export function isComposingTarget(target: EventTarget | null) { return !!target && composingInputs.has(target); }
export function resetComposition(target: EventTarget) { composingInputs.delete(target); composingEnters.delete(target); }
export function isCompositionEvent(event: Event) {
  return !!(event as KeyboardEvent | InputEvent).isComposing || (event as KeyboardEvent).keyCode === 229 || isComposingTarget(event.target) ||
    (event instanceof KeyboardEvent && isEnter(event) && !!event.target && composingEnters.has(event.target));
}
