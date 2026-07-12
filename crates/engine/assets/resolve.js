function (ref) {
  if (!window.__truewright || !window.__truewright.refs) {
    return { ok: false };
  }
  var el = window.__truewright.refs.get(ref);
  if (!el || !el.isConnected) {
    return { ok: false };
  }

  var style = window.getComputedStyle(el);
  var rect = el.getBoundingClientRect();
  var visible = style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0;

  if (visible) {
    el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
    // Scrolling the element into view only guarantees it's visible within
    // its own frame's viewport -- if an ancestor <iframe> is itself
    // scrolled out of view in ITS parent, the element could still be
    // unreachable. Walk the same frame chain a second time so every level
    // ends up in view before the final rect is read.
    var scrollWin = el.ownerDocument.defaultView;
    while (scrollWin.frameElement) {
      scrollWin.frameElement.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      scrollWin = scrollWin.frameElement.ownerDocument.defaultView;
    }
    rect = el.getBoundingClientRect();
  }

  // getBoundingClientRect() is relative to the element's OWN frame's
  // viewport, but CDP's Input.dispatchMouseEvent (what every click/type
  // ultimately goes through) expects top-level-page viewport coordinates.
  // Accumulate each ancestor <iframe>'s own rect (in ITS OWN parent's
  // coordinate space) to translate up to the top (same-origin-iframes
  // spec: "Correct click/type coordinates for elements inside a
  // same-origin iframe").
  var x = rect.x;
  var y = rect.y;
  var win = el.ownerDocument.defaultView;
  while (win.frameElement) {
    var frameRect = win.frameElement.getBoundingClientRect();
    x += frameRect.x;
    y += frameRect.y;
    win = win.frameElement.ownerDocument.defaultView;
  }

  return {
    ok: true,
    visible: visible,
    x: x + rect.width / 2,
    y: y + rect.height / 2,
    width: rect.width,
    height: rect.height,
  };
}
