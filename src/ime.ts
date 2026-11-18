// One document-lifetime tracker shared by all Desktop input handlers.
// Weak targets do not retain disposed conversations or terminal textareas.
const composingInputs = new WeakSet<EventTarget>();
let composingEnters = new WeakSet<EventTarget>();
let opaqueCompositionKeys = new WeakSet<EventTarget>();
document.addEventListener('compositionstart', event => { if (event.target) composingInputs.add(event.target); }, true);
document.addEventListener('compositionend', event => {
  if (!event.target) return;
  composingInputs.delete(event.target);
  // WebView2 may report the confirming press only as Process/229 with no code.
  // A character released before compositionend must not guard the next Enter.
  if (opaqueCompositionKeys.has(event.target)) composingEnters.add(event.target);
}, true);
document.addEventListener('focusout', event => { if (event.target) resetComposition(event.target); }, true);
const isEnter = (event: KeyboardEvent) => event.key === 'Enter' || event.code === 'Enter' || event.code === 'NumpadEnter';
const isModifier = (event: KeyboardEvent) => ['Shift', 'Control', 'Alt', 'Meta', 'AltGraph'].includes(event.key);
document.addEventListener('keydown', event => {
  if (!event.target) return;
  if (!isEnter(event)) {
    if (isModifier(event)) return;
    composingEnters.delete(event.target);
    if (!event.code && (event.key === 'Process' || event.keyCode === 229) &&
        (event.isComposing || isComposingTarget(event.target))) opaqueCompositionKeys.add(event.target);
    else opaqueCompositionKeys.delete(event.target);
    return;
  }
  // Windows can emit Process/229, compositionend, then Enter/13 for the
  // same physical press. Keep that press guarded until release, not a timer.
  if (event.isComposing || event.keyCode === 229 || isComposingTarget(event.target)) {
    composingEnters.add(event.target);
  // Leave the IME's own events alone; suppress only the trailing browser action.
  } else if (composingEnters.has(event.target)) {
    event.preventDefault();
    // The opaque press is now identified as Enter. An overlapping character's
    // keyup must not clear its guard; ordinary keydown and focus resets remain.
    opaqueCompositionKeys.delete(event.target);
  }
}, true);
document.addEventListener('keyup', event => {
  if (!event.target) return;
  // An opaque press can become Process, Enter, or a resolved character at release.
  const opaqueReleased = opaqueCompositionKeys.has(event.target) && !isModifier(event);
  if (isEnter(event) || opaqueReleased) {
    composingEnters.delete(event.target);
    opaqueCompositionKeys.delete(event.target);
  }
}, true);
window.addEventListener('blur', () => {
  composingEnters = new WeakSet<EventTarget>();
  opaqueCompositionKeys = new WeakSet<EventTarget>();
});

export function isComposingTarget(target: EventTarget | null) { return !!target && composingInputs.has(target); }
export function resetComposition(target: EventTarget) {
  composingInputs.delete(target); composingEnters.delete(target); opaqueCompositionKeys.delete(target);
}
export function isCompositionEvent(event: Event) {
  return !!(event as KeyboardEvent | InputEvent).isComposing || (event as KeyboardEvent).keyCode === 229 || isComposingTarget(event.target) ||
    (event instanceof KeyboardEvent && isEnter(event) && !!event.target && composingEnters.has(event.target));
}
