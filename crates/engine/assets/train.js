function (bindingName, action) {
  if (action === 'stop') {
    if (window.__aibTrain) {
      window.__aibTrain.handlers.forEach(function (h) {
        document.removeEventListener(h.type, h.fn, true);
      });
      window.__aibTrain = null;
    }
    return { ok: true };
  }

  if (window.__aibTrain) {
    return { ok: false, reason: 'already training' };
  }

  function report(sample) {
    // window.__aibSuppressTraining is toggled by this engine's own
    // click/type/press dispatch for the duration of each action while
    // training is active -- CDP-dispatched Input.dispatch*Event calls are
    // themselves isTrusted === true in Chrome (verified: isTrusted does NOT
    // distinguish real hardware input from CDP-synthesized input), so
    // isTrusted alone cannot exclude this engine's own dispatch. The
    // suppress flag is the actual guard; isTrusted still excludes a page's
    // own untrusted JS-dispatched events (human-motion spec: "Synthetic
    // dispatch is not captured as training data").
    if (window.__aibSuppressTraining) return;
    window[bindingName](JSON.stringify(sample));
  }

  function onMouseMove(e) {
    if (!e.isTrusted) return;
    report({ type: 'mousemove', x: e.clientX, y: e.clientY, t: performance.now() });
  }
  function onMouseDown(e) {
    if (!e.isTrusted) return;
    report({ type: 'mousedown', x: e.clientX, y: e.clientY, t: performance.now() });
  }
  function onMouseUp(e) {
    if (!e.isTrusted) return;
    report({ type: 'mouseup', x: e.clientX, y: e.clientY, t: performance.now() });
  }
  function onKeyDown(e) {
    if (!e.isTrusted) return;
    report({ type: 'keydown', key: e.key, t: performance.now() });
  }
  function onKeyUp(e) {
    if (!e.isTrusted) return;
    report({ type: 'keyup', key: e.key, t: performance.now() });
  }

  var handlers = [
    { type: 'mousemove', fn: onMouseMove },
    { type: 'mousedown', fn: onMouseDown },
    { type: 'mouseup', fn: onMouseUp },
    { type: 'keydown', fn: onKeyDown },
    { type: 'keyup', fn: onKeyUp },
  ];
  handlers.forEach(function (h) {
    document.addEventListener(h.type, h.fn, true);
  });

  window.__aibTrain = { handlers: handlers };
  return { ok: true };
}
