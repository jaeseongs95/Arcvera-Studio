// One document-lifetime tracker shared by all Desktop input handlers.
// Weak targets do not retain disposed conversations or terminal textareas.
let composingInputs = new WeakSet<EventTarget>();
let composingEnters = new WeakSet<EventTarget>();
let opaqueCompositionKeys = new WeakMap<EventTarget, number>();
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
  const target = event.target;
  if (!target) return;
  const nativeComposition = event.isComposing || event.keyCode === 229;
  const composing = isComposingTarget(target);
  if (event.key === 'Escape' && composing && !nativeComposition) {
    // The browser would also dismiss a modal when its handler ignores Escape.
    event.preventDefault();
    return;
  }
  if (!isEnter(event)) {
    if (isModifier(event)) return;
    composingEnters.delete(target);
    if (!event.code && (event.key === 'Process' || event.keyCode === 229) &&
        (nativeComposition || composing)) {
      const held = opaqueCompositionKeys.get(target) ?? 0;
      if (!event.repeat || held === 0) opaqueCompositionKeys.set(target, held + 1);
    } else opaqueCompositionKeys.delete(target);
    return;
  }
  // Windows can emit Process/229, compositionend, then Enter/13 for the
  // same physical press. Keep that press guarded until release, not a timer.
  if (nativeComposition || composing || composingEnters.has(target)) {
    composingEnters.add(target);
    // An unmarked key can also submit a form before compositionend.
    if (!nativeComposition) {
      event.preventDefault();
      // Once identified as Enter, wait for Enter release instead of Process.
      if (!composing) opaqueCompositionKeys.delete(target);
    }
  }
}, true);
document.addEventListener('keyup', event => {
  const target = event.target;
  if (!target || isModifier(event)) return;
  // An opaque press can become Process, Enter, or a resolved character at release.
  const held = opaqueCompositionKeys.get(target) ?? 0;
  if (isEnter(event) || held === 1) {
    composingEnters.delete(target);
    opaqueCompositionKeys.delete(target);
  } else if (held > 1) {
    opaqueCompositionKeys.set(target, held - 1);
  }
}, true);
window.addEventListener('blur', () => {
  composingInputs = new WeakSet<EventTarget>();
  composingEnters = new WeakSet<EventTarget>();
  opaqueCompositionKeys = new WeakMap<EventTarget, number>();
});

export function isComposingTarget(target: EventTarget | null) { return !!target && composingInputs.has(target); }
export function resetComposition(target: EventTarget) {
  composingInputs.delete(target); composingEnters.delete(target); opaqueCompositionKeys.delete(target);
}
export function isCompositionEvent(event: Event) {
  return !!(event as KeyboardEvent | InputEvent).isComposing || (event as KeyboardEvent).keyCode === 229 || isComposingTarget(event.target) ||
    (event instanceof KeyboardEvent && isEnter(event) && !!event.target && composingEnters.has(event.target));
}
