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

  function walkIframe(el, depth) {
    // Every iframe gets a ref, same-origin or not -- a cross-origin one may
    // still be backed by an attachable OOPIF target, and the Rust-side
    // correlation step (cross-origin-oopif spec) needs a ref on THIS node
    // to find it and splice the OOPIF's own content in as its children.
    var ref = refFor(el);

    var doc = null;
    try {
      doc = el.contentDocument;
    } catch (e) {
      doc = null;
    }

    if (!doc) {
      return {
        tag: 'iframe',
        role: 'iframe',
        ref: ref,
        name: 'cross-origin iframe (not inspectable): ' + (el.getAttribute('src') || ''),
        children: [],
      };
    }
    if (!doc.body) {
      return { tag: 'iframe', role: 'iframe', ref: ref, name: 'iframe not yet loaded', children: [] };
    }

    var child = walk(doc.body, depth + 1);
    var out = { tag: 'iframe', role: 'iframe', ref: ref, children: child ? [child] : [] };
    var title = el.getAttribute('title');
    if (title) out.name = title;
    return out;
  }

  function walk(el, depth) {
    if (depth > 40 || el.nodeType !== 1 || SKIP_TAGS.has(el.tagName)) return null;
    if (el.tagName === 'IFRAME') return walkIframe(el, depth);

    var interactive = INTERACTIVE_TAGS.has(el.tagName) || el.hasAttribute('role') || typeof el.onclick === 'function';
    var structural = STRUCTURAL_TAGS.has(el.tagName);
    var visible = isVisible(el);

    // An open shadow root's content is a separate tree from el.children
    // (light DOM) -- walk it instead, not in addition, since light-DOM
    // children are either projected via a matching <slot> (reachable
    // through the shadow tree itself, below) or not rendered at all.
    // Closed shadow roots report shadowRoot === null indistinguishably
    // from "no shadow root" -- genuinely undetectable from script, so
    // they fall through to the light-DOM case below (shadow-dom-walker
    // spec: "Refs and clicks work on shadow-tree elements").
    var childSource;
    if (el.tagName === 'SLOT') {
      var assigned = el.assignedElements ? el.assignedElements() : [];
      childSource = assigned.length ? assigned : el.children;
    } else if (el.shadowRoot) {
      childSource = el.shadowRoot.children;
    } else {
      childSource = el.children;
    }

    var children = [];
    for (var i = 0; i < childSource.length; i++) {
      var node = walk(childSource[i], depth + 1);
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
