(function () {
  window.__aib = window.__aib || { refs: new Map(), elRefs: new WeakMap(), counter: 0 };

  var INTERACTIVE_TAGS = new Set(['A', 'BUTTON', 'INPUT', 'TEXTAREA', 'SELECT', 'SUMMARY']);
  var STRUCTURAL_TAGS = new Set(['MAIN', 'NAV', 'HEADER', 'FOOTER', 'FORM', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6']);
  var SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE']);
  // Landmark containers wrap arbitrary subtrees (often including other named
  // elements), so falling back to their full textContent for a name would
  // swallow everything inside them. Headings are excluded here on purpose:
  // a heading's own text IS its name.
  var LANDMARK_TAGS = new Set(['MAIN', 'NAV', 'HEADER', 'FOOTER', 'FORM']);

  function inputRole(el) {
    var type = (el.getAttribute('type') || 'text').toLowerCase();
    var map = { checkbox: 'checkbox', radio: 'radio', button: 'button', submit: 'button', range: 'slider' };
    return map[type] || 'textbox';
  }

  function role(el) {
    var explicit = el.getAttribute('role');
    if (explicit) return explicit;
    var tag = el.tagName;
    switch (tag) {
      case 'A':
        return el.hasAttribute('href') ? 'link' : 'generic';
      case 'BUTTON':
        return 'button';
      case 'INPUT':
        return inputRole(el);
      case 'TEXTAREA':
        return 'textbox';
      case 'SELECT':
        return el.multiple ? 'listbox' : 'combobox';
      case 'H1': case 'H2': case 'H3': case 'H4': case 'H5': case 'H6':
        return 'heading';
      case 'NAV':
        return 'navigation';
      case 'MAIN':
        return 'main';
      case 'HEADER':
        return 'banner';
      case 'FOOTER':
        return 'contentinfo';
      case 'FORM':
        return 'form';
      case 'SUMMARY':
        return 'button';
      default:
        return 'generic';
    }
  }

  function accessibleName(el) {
    var aria = el.getAttribute('aria-label');
    if (aria) return aria.trim();

    var labelledby = el.getAttribute('aria-labelledby');
    if (labelledby) {
      var joined = labelledby
        .split(/\s+/)
        .map(function (id) {
          var t = document.getElementById(id);
          return t ? t.textContent.trim() : '';
        })
        .filter(Boolean)
        .join(' ');
      if (joined) return joined;
    }

    if (el.tagName === 'IMG') return (el.getAttribute('alt') || '').trim();

    if (el.labels && el.labels.length) {
      var fromLabels = Array.from(el.labels).map(function (l) { return l.textContent.trim(); }).join(' ');
      if (fromLabels) return fromLabels;
    }

    if ((el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') && el.placeholder) {
      return el.placeholder.trim();
    }

    if (LANDMARK_TAGS.has(el.tagName)) return '';

    var text = (el.textContent || '').trim().replace(/\s+/g, ' ');
    return text.slice(0, 200);
  }

  function ownText(el) {
    var text = '';
    for (var i = 0; i < el.childNodes.length; i++) {
      var node = el.childNodes[i];
      if (node.nodeType === 3) text += node.textContent;
    }
    return text.trim().replace(/\s+/g, ' ');
  }

  function isVisible(el) {
    var style = window.getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
    var rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  function refFor(el) {
    var ref = window.__aib.elRefs.get(el);
    if (ref) return ref;
    ref = 'e' + (++window.__aib.counter);
    window.__aib.elRefs.set(el, ref);
    window.__aib.refs.set(ref, el);
    return ref;
  }

  function walk(el, depth) {
    if (depth > 40 || el.nodeType !== 1 || SKIP_TAGS.has(el.tagName)) return null;

    var interactive = INTERACTIVE_TAGS.has(el.tagName) || el.hasAttribute('role') || typeof el.onclick === 'function';
    var structural = STRUCTURAL_TAGS.has(el.tagName);
    var visible = isVisible(el);

    var children = [];
    for (var i = 0; i < el.children.length; i++) {
      var node = walk(el.children[i], depth + 1);
      if (node) children.push(node);
    }

    var text = ownText(el);
    var isTextLeaf = !interactive && !structural && children.length === 0 && text.length > 0;

    if (!interactive && !structural && !isTextLeaf && children.length === 0) return null;
    if (!visible && children.length === 0 && !isTextLeaf) return null;

    var out = {
      tag: el.tagName.toLowerCase(),
      role: (interactive || structural) ? role(el) : (isTextLeaf ? 'text' : 'generic'),
      children: children,
    };

    if (interactive || structural || isTextLeaf) {
      if (interactive || structural) out.ref = refFor(el);
      var name = isTextLeaf ? text : accessibleName(el);
      if (name) out.name = name;
      if (!visible) out.hidden = true;
      if (el.disabled) out.disabled = true;
      if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') out.value = el.value;
      if (el.tagName === 'INPUT' && (el.type === 'checkbox' || el.type === 'radio')) out.checked = el.checked;
    }

    return out;
  }

  return {
    url: location.href,
    title: document.title,
    tree: walk(document.body, 0),
  };
})()
