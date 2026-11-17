// One document-lifetime tracker shared by all Desktop input handlers.
// Weak targets do not retain disposed conversations or terminal textareas.
const composingInputs = new WeakSet<EventTarget>();
document.addEventListener('compositionstart', event => { if (event.target) composingInputs.add(event.target); }, true);
document.addEventListener('compositionend', event => { if (event.target) composingInputs.delete(event.target); }, true);
document.addEventListener('focusout', event => { if (event.target) composingInputs.delete(event.target); }, true);

export function isComposingTarget(target: EventTarget | null) { return !!target && composingInputs.has(target); }
export function resetComposition(target: EventTarget) { composingInputs.delete(target); }
export function isCompositionEvent(event: Event) {
  return !!(event as KeyboardEvent | InputEvent).isComposing || (event as KeyboardEvent).keyCode === 229 || isComposingTarget(event.target);
}
