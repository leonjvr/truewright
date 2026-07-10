function (ref) {
  if (!window.__aib || !window.__aib.refs) {
    return { ok: false };
  }
  var el = window.__aib.refs.get(ref);
  if (!el || !el.isConnected) {
    return { ok: false };
  }

  var style = window.getComputedStyle(el);
  var rect = el.getBoundingClientRect();
  var visible = style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0;

  if (visible) {
    el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
    rect = el.getBoundingClientRect();
  }

  return {
    ok: true,
    visible: visible,
    x: rect.x + rect.width / 2,
    y: rect.y + rect.height / 2,
    width: rect.width,
    height: rect.height,
  };
}
