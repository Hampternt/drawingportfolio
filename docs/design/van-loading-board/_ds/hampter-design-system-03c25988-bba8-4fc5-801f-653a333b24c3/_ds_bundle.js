/* @ds-bundle: {"format":4,"namespace":"HampterDesignSystem_03c259","components":[{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Card","sourcePath":"components/core/Card.jsx"},{"name":"HubCard","sourcePath":"components/core/HubCard.jsx"},{"name":"Icon","sourcePath":"components/core/Icon.jsx"},{"name":"IconButton","sourcePath":"components/core/IconButton.jsx"},{"name":"Kbd","sourcePath":"components/core/Kbd.jsx"},{"name":"KbdGroup","sourcePath":"components/core/Kbd.jsx"},{"name":"PostCard","sourcePath":"components/core/PostCard.jsx"},{"name":"Tag","sourcePath":"components/core/Tag.jsx"},{"name":"CalorieRing","sourcePath":"components/data/CalorieRing.jsx"},{"name":"MacroRail","sourcePath":"components/data/MacroRail.jsx"},{"name":"Dialog","sourcePath":"components/feedback/Dialog.jsx"},{"name":"Toast","sourcePath":"components/feedback/Toast.jsx"},{"name":"ToastStack","sourcePath":"components/feedback/Toast.jsx"},{"name":"Tooltip","sourcePath":"components/feedback/Tooltip.jsx"},{"name":"Checkbox","sourcePath":"components/forms/Checkbox.jsx"},{"name":"Input","sourcePath":"components/forms/Input.jsx"},{"name":"Textarea","sourcePath":"components/forms/Input.jsx"},{"name":"Radio","sourcePath":"components/forms/Radio.jsx"},{"name":"Select","sourcePath":"components/forms/Select.jsx"},{"name":"Switch","sourcePath":"components/forms/Switch.jsx"},{"name":"CommandPalette","sourcePath":"components/navigation/CommandPalette.jsx"},{"name":"ShortcutsOverlay","sourcePath":"components/navigation/ShortcutsOverlay.jsx"},{"name":"RESERVED_SHORTCUTS","sourcePath":"components/navigation/ShortcutsOverlay.jsx"},{"name":"Tabs","sourcePath":"components/navigation/Tabs.jsx"}],"sourceHashes":{"components/core/Badge.jsx":"32562b81b7c6","components/core/Button.jsx":"fad4371302ea","components/core/Card.jsx":"f3d98de80899","components/core/HubCard.jsx":"41871e177645","components/core/Icon.jsx":"4a5d838a90d5","components/core/IconButton.jsx":"f452dff99290","components/core/Kbd.jsx":"375d8cb505f3","components/core/PostCard.jsx":"9b1dcfa163ef","components/core/Tag.jsx":"5de7e5cd51d6","components/data/CalorieRing.jsx":"2d3922869354","components/data/MacroRail.jsx":"01f3919bd3a0","components/feedback/Dialog.jsx":"ed05ca607e67","components/feedback/Toast.jsx":"d21a61d0ed05","components/feedback/Tooltip.jsx":"fe42a096e07e","components/forms/Checkbox.jsx":"08bccbfd4ae9","components/forms/Input.jsx":"44ca018e44d7","components/forms/Radio.jsx":"3e494a32c54c","components/forms/Select.jsx":"4d23bc149e55","components/forms/Switch.jsx":"f17d42d88c44","components/navigation/CommandPalette.jsx":"c133c68048b5","components/navigation/ShortcutsOverlay.jsx":"fbe13a98c1a5","components/navigation/Tabs.jsx":"f3dcc4b622e7","ui_kits/portfolio/ArtScreen.jsx":"ad0297560381","ui_kits/portfolio/FitnessScreen.jsx":"08d5d6a57d3a","ui_kits/portfolio/HubScreen.jsx":"a03038834e4c","ui_kits/portfolio/SiteChrome.jsx":"b543d1a82142","ui_kits/portfolio/TasksScreen.jsx":"e6d5b3a8069f","ui_kits/portfolio/data.js":"ad5398fa4e05"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.HampterDesignSystem_03c259 = window.HampterDesignSystem_03c259 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/core/Badge.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Small uppercase status marker: build state, category, availability. */
function Badge({
  tone = "neutral",
  dot = false,
  className = "",
  children,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("span", _extends({
    className: ["hm-badge", "hm-badge--" + tone, className].filter(Boolean).join(" ")
  }, rest), dot ? /*#__PURE__*/React.createElement("span", {
    className: "hm-badge__dot"
  }) : null, children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Badge.jsx", error: String((e && e.message) || e) }); }

// components/core/Card.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Bordered panel. Elevation on dark comes from the border, not a shadow. */
function Card({
  variant = "default",
  interactive = false,
  title,
  actions,
  footer,
  className = "",
  children,
  ...rest
}) {
  const cls = ["hm-card", variant !== "default" ? "hm-card--" + variant : "", interactive ? "hm-card--interactive" : "", className].filter(Boolean).join(" ");
  return /*#__PURE__*/React.createElement("div", _extends({
    className: cls,
    tabIndex: interactive ? 0 : undefined
  }, rest), title ? /*#__PURE__*/React.createElement("div", {
    className: "hm-card__header"
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-card__title"
  }, title), actions ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: "auto",
      display: "flex",
      gap: "var(--space-3)"
    }
  }, actions) : null) : null, /*#__PURE__*/React.createElement("div", {
    className: "hm-card__body"
  }, children), footer ? /*#__PURE__*/React.createElement("div", {
    className: "hm-card__footer"
  }, footer) : null);
}
Object.assign(__ds_scope, { Card });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Card.jsx", error: String((e && e.message) || e) }); }

// components/core/Icon.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const CDN = "https://unpkg.com/lucide-static@0.462.0/icons/";

/** Lucide glyph rendered as a CSS mask so it inherits currentColor. */
function Icon({
  name,
  size = 16,
  strokeWidth,
  className = "",
  style,
  ...rest
}) {
  const url = "url(" + CDN + name + ".svg)";
  return /*#__PURE__*/React.createElement("span", _extends({
    "aria-hidden": "true",
    className: "hm-icon " + className,
    style: {
      width: size,
      height: size,
      WebkitMaskImage: url,
      maskImage: url,
      ...style
    },
    "data-icon": name,
    "data-stroke": strokeWidth
  }, rest));
}
Object.assign(__ds_scope, { Icon });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Icon.jsx", error: String((e && e.message) || e) }); }

// components/core/HubCard.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** A section tile on the index page — one per surface of the site. */
function HubCard({
  icon,
  title,
  description,
  meta,
  shortcut,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("a", _extends({
    className: ["hm-hub", className].filter(Boolean).join(" ")
  }, rest), /*#__PURE__*/React.createElement("div", {
    className: "hm-hub__top"
  }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 18
  }) : null, /*#__PURE__*/React.createElement("span", {
    className: "hm-hub__title"
  }, title), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      color: "var(--text-faint)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "arrow-up-right",
    size: 15
  }))), description ? /*#__PURE__*/React.createElement("p", {
    className: "hm-hub__desc"
  }, description) : null, meta || shortcut ? /*#__PURE__*/React.createElement("div", {
    className: "hm-hub__foot"
  }, meta, shortcut ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, shortcut) : null) : null);
}
Object.assign(__ds_scope, { HubCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/HubCard.jsx", error: String((e && e.message) || e) }); }

// components/core/IconButton.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Square, label-less button for toolbars and rails. Always pass a label for a11y. */
function IconButton({
  icon,
  label,
  variant = "ghost",
  size = "md",
  active = false,
  disabled = false,
  className = "",
  ...rest
}) {
  const cls = ["hm-iconbtn", "hm-iconbtn--" + size, "hm-iconbtn--" + variant, active ? "is-active" : "", className].filter(Boolean).join(" ");
  return /*#__PURE__*/React.createElement("button", _extends({
    type: "button",
    className: cls,
    "aria-label": label,
    title: label,
    disabled: disabled
  }, rest), /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: size === "sm" ? 14 : size === "lg" ? 18 : 16
  }));
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/core/Kbd.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** A keycap. The brand's signature detail — shortcuts are shown, never hidden. */
function Kbd({
  size = "md",
  className = "",
  children,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("kbd", _extends({
    className: ["hm-kbd", size === "lg" ? "hm-kbd--lg" : "", className].filter(Boolean).join(" ")
  }, rest), children);
}

/** A sequence of keycaps with an optional trailing hint, e.g. ⌘ K  open palette. */
function KbdGroup({
  keys = [],
  hint,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("span", _extends({
    className: ["hm-kbd-group", className].filter(Boolean).join(" ")
  }, rest), keys.map((k, i) => /*#__PURE__*/React.createElement(Kbd, {
    key: i
  }, k)), hint ? /*#__PURE__*/React.createElement("span", null, hint) : null);
}
Object.assign(__ds_scope, { Kbd, KbdGroup });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Kbd.jsx", error: String((e && e.message) || e) }); }

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Primary action control. Renders as <button> or, with href, as <a>. */
function Button({
  variant = "secondary",
  size = "md",
  icon,
  iconRight,
  shortcut,
  block = false,
  disabled = false,
  href,
  className = "",
  children,
  ...rest
}) {
  const Tag = href ? "a" : "button";
  const cls = ["hm-btn", "hm-btn--" + variant, "hm-btn--" + size, block ? "hm-btn--block" : "", className].filter(Boolean).join(" ");
  const glyph = size === "lg" ? 16 : 14;
  return /*#__PURE__*/React.createElement(Tag, _extends({
    className: cls,
    href: href,
    disabled: href ? undefined : disabled,
    "aria-disabled": disabled || undefined
  }, rest), icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: glyph
  }) : null, children, iconRight ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: iconRight,
    size: glyph
  }) : null, shortcut ? /*#__PURE__*/React.createElement(__ds_scope.Kbd, {
    className: "hm-btn__kbd"
  }, shortcut) : null);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/core/PostCard.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** One drawing in the art portfolio feed: image, caption, date. */
function PostCard({
  image,
  caption,
  date,
  badge,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("article", _extends({
    className: ["hm-post", className].filter(Boolean).join(" ")
  }, rest), /*#__PURE__*/React.createElement("div", {
    className: "hm-post__media"
  }, image ? /*#__PURE__*/React.createElement("img", {
    src: image,
    alt: caption || "",
    loading: "lazy"
  }) : null), /*#__PURE__*/React.createElement("div", {
    className: "hm-post__body"
  }, caption ? /*#__PURE__*/React.createElement("p", {
    className: "hm-post__caption"
  }, caption) : null, /*#__PURE__*/React.createElement("div", {
    className: "hm-post__meta"
  }, /*#__PURE__*/React.createElement("span", null, date), badge ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, badge) : null)));
}
Object.assign(__ds_scope, { PostCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/PostCard.jsx", error: String((e && e.message) || e) }); }

// components/core/Tag.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Pill for taxonomy: tech stack, genre, hobby. Optionally filterable or removable. */
function Tag({
  selected = false,
  interactive = false,
  onRemove,
  className = "",
  children,
  ...rest
}) {
  const cls = ["hm-tag", interactive || onRemove ? "hm-tag--interactive" : "", selected ? "is-selected" : "", className].filter(Boolean).join(" ");
  const Tag_ = interactive ? "button" : "span";
  return /*#__PURE__*/React.createElement(Tag_, _extends({
    className: cls,
    type: interactive ? "button" : undefined,
    "aria-pressed": interactive ? selected : undefined
  }, rest), children, onRemove ? /*#__PURE__*/React.createElement("span", {
    className: "hm-tag__remove",
    role: "button",
    "aria-label": "Remove",
    onClick: onRemove
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "x",
    size: 12
  })) : null);
}
Object.assign(__ds_scope, { Tag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tag.jsx", error: String((e && e.message) || e) }); }

// components/data/CalorieRing.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Progress ring used for calories against a daily target. */
function CalorieRing({
  value = 0,
  target = 2000,
  size = 148,
  stroke = 10,
  label = "kcal left",
  className = "",
  ...rest
}) {
  const pct = Math.max(0, Math.min(1, target ? value / target : 0));
  const r = (size - stroke) / 2;
  const circ = 2 * Math.PI * r;
  const over = value > target;
  return /*#__PURE__*/React.createElement("div", _extends({
    className: ["hm-ring", className].filter(Boolean).join(" "),
    style: {
      width: size,
      height: size
    }
  }, rest), /*#__PURE__*/React.createElement("svg", {
    width: size,
    height: size,
    style: {
      transform: "rotate(-90deg)"
    }
  }, /*#__PURE__*/React.createElement("circle", {
    cx: size / 2,
    cy: size / 2,
    r: r,
    fill: "none",
    strokeWidth: stroke,
    stroke: "var(--white-a07)"
  }), /*#__PURE__*/React.createElement("circle", {
    cx: size / 2,
    cy: size / 2,
    r: r,
    fill: "none",
    strokeWidth: stroke,
    strokeLinecap: "round",
    stroke: over ? "var(--status-danger)" : "var(--accent)",
    strokeDasharray: circ,
    strokeDashoffset: circ * (1 - pct),
    style: {
      transition: "stroke-dashoffset var(--dur-slow) var(--ease-snap)"
    }
  })), /*#__PURE__*/React.createElement("span", {
    className: "hm-ring__val"
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-ring__num"
  }, Math.abs(target - value)), /*#__PURE__*/React.createElement("span", {
    className: "hm-ring__sub"
  }, label)));
}
Object.assign(__ds_scope, { CalorieRing });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/CalorieRing.jsx", error: String((e && e.message) || e) }); }

// components/data/MacroRail.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Thin progress rail for a macro (protein, carbs, fat) against its target. */
function MacroRail({
  label,
  value = 0,
  target = 0,
  unit = "g",
  color = "var(--accent)",
  className = "",
  ...rest
}) {
  const pct = target ? Math.max(0, Math.min(1, value / target)) : 0;
  return /*#__PURE__*/React.createElement("div", _extends({
    className: ["hm-rail", className].filter(Boolean).join(" ")
  }, rest), /*#__PURE__*/React.createElement("div", {
    className: "hm-rail__head"
  }, /*#__PURE__*/React.createElement("span", null, label), /*#__PURE__*/React.createElement("span", {
    className: "hm-rail__val"
  }, value, unit, " / ", target, unit)), /*#__PURE__*/React.createElement("div", {
    className: "hm-rail__track"
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-rail__fill",
    style: {
      width: pct * 100 + "%",
      background: color
    }
  })));
}
Object.assign(__ds_scope, { MacroRail });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/MacroRail.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Dialog.jsx
try { (() => {
/** Modal dialog. Confirmations and short forms only — never a second page. */
function Dialog({
  open = true,
  title,
  description,
  onClose,
  footer,
  confirmLabel,
  onConfirm,
  destructive = false,
  children,
  className = ""
}) {
  React.useEffect(() => {
    if (!open) return;
    const h = e => {
      if (e.key === "Escape" && onClose) onClose();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  if (!open) return null;
  return /*#__PURE__*/React.createElement("div", {
    className: "hm-scrim",
    onClick: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: ["hm-dialog", className].filter(Boolean).join(" "),
    role: "dialog",
    "aria-modal": "true",
    onClick: e => e.stopPropagation()
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-dialog__header"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1
    }
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-dialog__title"
  }, title), description ? /*#__PURE__*/React.createElement("div", {
    className: "hm-dialog__desc"
  }, description) : null)), children ? /*#__PURE__*/React.createElement("div", {
    className: "hm-dialog__body"
  }, children) : null, /*#__PURE__*/React.createElement("div", {
    className: "hm-dialog__footer"
  }, footer || /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(__ds_scope.Button, {
    variant: "ghost",
    onClick: onClose
  }, "Cancel"), confirmLabel ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    variant: destructive ? "danger" : "primary",
    onClick: onConfirm
  }, confirmLabel) : null))));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Toast.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const ICONS = {
  success: "check-circle-2",
  danger: "alert-triangle",
  info: "info",
  accent: "sparkles"
};

/** Transient confirmation. Bottom-right stack, one line of body text max. */
function Toast({
  tone = "info",
  title,
  description,
  onDismiss,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("div", _extends({
    className: ["hm-toast", "hm-toast--" + tone, className].filter(Boolean).join(" "),
    role: "status"
  }, rest), /*#__PURE__*/React.createElement("span", {
    className: "hm-toast__icon"
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: ICONS[tone] || "info",
    size: 16
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1
    }
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-toast__title"
  }, title), description ? /*#__PURE__*/React.createElement("div", {
    className: "hm-toast__desc"
  }, description) : null), onDismiss ? /*#__PURE__*/React.createElement("span", {
    className: "hm-toast__close"
  }, /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    icon: "x",
    label: "Dismiss",
    size: "sm",
    onClick: onDismiss
  })) : null);
}

/** Fixed bottom-right stack for Toasts. */
function ToastStack({
  children
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "fixed",
      right: "var(--space-8)",
      bottom: "var(--space-8)",
      zIndex: 90,
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-4)"
    }
  }, children);
}
Object.assign(__ds_scope, { Toast, ToastStack });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Toast.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Tooltip.jsx
try { (() => {
/** Hover/focus label. Include the shortcut whenever the target has one. */
function Tooltip({
  label,
  shortcut,
  side = "top",
  children,
  className = ""
}) {
  const [open, setOpen] = React.useState(false);
  return /*#__PURE__*/React.createElement("span", {
    className: ["hm-tooltip", className].filter(Boolean).join(" "),
    onMouseEnter: () => setOpen(true),
    onMouseLeave: () => setOpen(false),
    onFocus: () => setOpen(true),
    onBlur: () => setOpen(false)
  }, children, /*#__PURE__*/React.createElement("span", {
    className: "hm-tooltip__bubble hm-tooltip__bubble--" + side + (open ? " is-open" : ""),
    role: "tooltip"
  }, label, shortcut ? /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, shortcut) : null));
}
Object.assign(__ds_scope, { Tooltip });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Tooltip.jsx", error: String((e && e.message) || e) }); }

// components/forms/Checkbox.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Checkbox with optional description line. */
function Checkbox({
  label,
  description,
  disabled = false,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("label", {
    className: ["hm-choice", disabled ? "is-disabled" : "", className].filter(Boolean).join(" ")
  }, /*#__PURE__*/React.createElement("input", _extends({
    type: "checkbox",
    disabled: disabled
  }, rest)), /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__box"
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "check",
    size: 11
  })), /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__text"
  }, /*#__PURE__*/React.createElement("span", null, label), description ? /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__desc"
  }, description) : null));
}
Object.assign(__ds_scope, { Checkbox });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Checkbox.jsx", error: String((e && e.message) || e) }); }

// components/forms/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Single-line text field with optional leading icon and trailing keycap. */
function Input({
  label,
  hint,
  error,
  icon,
  shortcut,
  size = "md",
  disabled = false,
  className = "",
  mono = false,
  id,
  ...rest
}) {
  const wrap = ["hm-input-wrap", size !== "md" ? "hm-input-wrap--" + size : "", error ? "is-invalid" : "", disabled ? "is-disabled" : ""].filter(Boolean).join(" ");
  return /*#__PURE__*/React.createElement("div", {
    className: ["hm-field", className].filter(Boolean).join(" ")
  }, label ? /*#__PURE__*/React.createElement("label", {
    className: "hm-field__label",
    htmlFor: id
  }, label) : null, /*#__PURE__*/React.createElement("div", {
    className: wrap
  }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 14
  }) : null, /*#__PURE__*/React.createElement("input", _extends({
    id: id,
    className: "hm-input" + (mono ? " hm-input--mono" : ""),
    disabled: disabled
  }, rest)), shortcut ? /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, shortcut) : null), error ? /*#__PURE__*/React.createElement("span", {
    className: "hm-field__hint hm-field__hint--error"
  }, error) : hint ? /*#__PURE__*/React.createElement("span", {
    className: "hm-field__hint"
  }, hint) : null);
}

/** Multi-line variant of Input. */
function Textarea({
  label,
  hint,
  error,
  className = "",
  id,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: ["hm-field", className].filter(Boolean).join(" ")
  }, label ? /*#__PURE__*/React.createElement("label", {
    className: "hm-field__label",
    htmlFor: id
  }, label) : null, /*#__PURE__*/React.createElement("textarea", _extends({
    id: id,
    className: "hm-textarea"
  }, rest)), error ? /*#__PURE__*/React.createElement("span", {
    className: "hm-field__hint hm-field__hint--error"
  }, error) : hint ? /*#__PURE__*/React.createElement("span", {
    className: "hm-field__hint"
  }, hint) : null);
}
Object.assign(__ds_scope, { Input, Textarea });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Input.jsx", error: String((e && e.message) || e) }); }

// components/forms/Radio.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Radio option. Group them by sharing a `name`. */
function Radio({
  label,
  description,
  disabled = false,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("label", {
    className: ["hm-choice", disabled ? "is-disabled" : "", className].filter(Boolean).join(" ")
  }, /*#__PURE__*/React.createElement("input", _extends({
    type: "radio",
    disabled: disabled
  }, rest)), /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__box hm-choice__box--radio"
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__dot"
  })), /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__text"
  }, /*#__PURE__*/React.createElement("span", null, label), description ? /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__desc"
  }, description) : null));
}
Object.assign(__ds_scope, { Radio });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Radio.jsx", error: String((e && e.message) || e) }); }

// components/forms/Select.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Native select in Hampter chrome. Options are {value,label} or plain strings. */
function Select({
  label,
  hint,
  options = [],
  disabled = false,
  className = "",
  id,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: ["hm-field", className].filter(Boolean).join(" ")
  }, label ? /*#__PURE__*/React.createElement("label", {
    className: "hm-field__label",
    htmlFor: id
  }, label) : null, /*#__PURE__*/React.createElement("div", {
    className: "hm-select" + (disabled ? " is-disabled" : "")
  }, /*#__PURE__*/React.createElement("select", _extends({
    id: id,
    disabled: disabled
  }, rest), options.map(o => {
    const value = typeof o === "string" ? o : o.value;
    const text = typeof o === "string" ? o : o.label;
    return /*#__PURE__*/React.createElement("option", {
      key: value,
      value: value
    }, text);
  })), /*#__PURE__*/React.createElement("span", {
    className: "hm-select__chevron"
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "chevron-down",
    size: 14
  }))), hint ? /*#__PURE__*/React.createElement("span", {
    className: "hm-field__hint"
  }, hint) : null);
}
Object.assign(__ds_scope, { Select });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Select.jsx", error: String((e && e.message) || e) }); }

// components/forms/Switch.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Boolean toggle for settings that apply immediately (theme, motion, CRT filter). */
function Switch({
  label,
  description,
  disabled = false,
  className = "",
  ...rest
}) {
  return /*#__PURE__*/React.createElement("label", {
    className: ["hm-choice", disabled ? "is-disabled" : "", className].filter(Boolean).join(" "),
    style: {
      alignItems: "center",
      gap: "var(--space-5)"
    }
  }, /*#__PURE__*/React.createElement("input", _extends({
    type: "checkbox",
    role: "switch",
    disabled: disabled
  }, rest)), /*#__PURE__*/React.createElement("span", {
    className: "hm-switch__track"
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-switch__thumb"
  })), label ? /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__text"
  }, /*#__PURE__*/React.createElement("span", null, label), description ? /*#__PURE__*/React.createElement("span", {
    className: "hm-choice__desc"
  }, description) : null) : null);
}
Object.assign(__ds_scope, { Switch });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Switch.jsx", error: String((e && e.message) || e) }); }

// components/navigation/CommandPalette.jsx
try { (() => {
/**
 * The site's primary navigation surface — ⌘K opens it everywhere.
 * Controlled: pass `open` and `onClose`. Commands are grouped by `group`.
 */
function CommandPalette({
  open = true,
  commands = [],
  isAdmin = false,
  placeholder = "Type a command or search\u2026",
  onClose,
  onRun,
  className = ""
}) {
  const [query, setQuery] = React.useState("");
  const [cursor, setCursor] = React.useState(0);
  const inputRef = React.useRef(null);
  const returnFocusTo = React.useRef(null);
  const results = React.useMemo(() => {
    const q = query.trim().toLowerCase();
    return commands.filter(c => !c.adminOnly || isAdmin).filter(c => !q || (c.label + " " + (c.group || "") + " " + (c.keywords || "")).toLowerCase().includes(q));
  }, [commands, query, isAdmin]);
  React.useEffect(() => {
    setCursor(0);
  }, [query]);
  React.useEffect(() => {
    if (open) {
      returnFocusTo.current = document.activeElement;
      if (inputRef.current) inputRef.current.focus();
    } else if (returnFocusTo.current && returnFocusTo.current.focus) {
      returnFocusTo.current.focus();
    }
  }, [open]);
  if (!open) return null;
  const onKeyDown = e => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setCursor(c => Math.min(c + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setCursor(c => Math.max(c - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results[cursor];
      if (r && onRun) onRun(r);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (onClose) onClose();
    }
  };
  const groups = [];
  results.forEach(r => {
    const g = r.group || "";
    const last = groups[groups.length - 1];
    if (!last || last.name !== g) groups.push({
      name: g,
      items: [r]
    });else last.items.push(r);
  });
  let i = -1;
  return /*#__PURE__*/React.createElement("div", {
    className: "hm-scrim",
    style: {
      alignItems: "flex-start",
      paddingTop: "14vh"
    },
    onClick: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: ["hm-cmdk", className].filter(Boolean).join(" "),
    role: "dialog",
    "aria-label": "Command palette",
    onClick: e => e.stopPropagation(),
    onKeyDown: onKeyDown
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-cmdk__input-row"
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "chevron-right",
    size: 16
  }), /*#__PURE__*/React.createElement("input", {
    ref: inputRef,
    className: "hm-cmdk__input",
    placeholder: placeholder,
    value: query,
    onChange: e => setQuery(e.target.value)
  }), /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "Esc")), /*#__PURE__*/React.createElement("div", {
    className: "hm-cmdk__list"
  }, results.length === 0 ? /*#__PURE__*/React.createElement("div", {
    className: "hm-cmdk__empty"
  }, "no matches for \u201C", query, "\u201D") : null, groups.map(g => /*#__PURE__*/React.createElement("div", {
    key: g.name
  }, g.name ? /*#__PURE__*/React.createElement("div", {
    className: "hm-cmdk__group"
  }, g.name) : null, g.items.map(r => {
    i += 1;
    const idx = i;
    return /*#__PURE__*/React.createElement("div", {
      key: r.id,
      className: "hm-cmdk__item",
      "aria-selected": idx === cursor,
      onMouseEnter: () => setCursor(idx),
      onClick: () => onRun && onRun(r)
    }, r.icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: r.icon,
      size: 15
    }) : null, /*#__PURE__*/React.createElement("span", null, r.label), r.hint ? /*#__PURE__*/React.createElement("span", {
      className: "hm-cmdk__item-hint"
    }, r.hint) : null);
  })))), /*#__PURE__*/React.createElement("div", {
    className: "hm-cmdk__footer"
  }, /*#__PURE__*/React.createElement("span", null, /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "\u2191"), " ", /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "\u2193"), " navigate"), /*#__PURE__*/React.createElement("span", null, /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "\u21B5"), " run"), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, results.length, " commands"))));
}
Object.assign(__ds_scope, { CommandPalette });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/CommandPalette.jsx", error: String((e && e.message) || e) }); }

// components/navigation/ShortcutsOverlay.jsx
try { (() => {
/**
 * The "?" overlay: every shortcut on the site, grouped. Doubles as the spec —
 * if a key is not listed here, it is not a shortcut.
 */
function ShortcutsOverlay({
  open = true,
  groups = [],
  onClose,
  className = ""
}) {
  React.useEffect(() => {
    if (!open) return;
    const h = e => {
      if (e.key === "Escape" && onClose) onClose();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  if (!open) return null;
  return /*#__PURE__*/React.createElement("div", {
    className: "hm-scrim",
    onClick: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: ["hm-shortcuts", className].filter(Boolean).join(" "),
    role: "dialog",
    "aria-label": "Keyboard shortcuts",
    onClick: e => e.stopPropagation()
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-shortcuts__head"
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-shortcuts__title"
  }, "Keyboard shortcuts"), /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    icon: "x",
    label: "Close",
    size: "sm",
    onClick: onClose
  })), /*#__PURE__*/React.createElement("div", {
    className: "hm-shortcuts__body"
  }, groups.map(g => /*#__PURE__*/React.createElement("div", {
    key: g.name,
    className: "hm-shortcuts__group"
  }, /*#__PURE__*/React.createElement("div", {
    className: "hm-shortcuts__group-name"
  }, g.name), g.items.map(it => /*#__PURE__*/React.createElement("div", {
    key: it.label,
    className: "hm-shortcuts__row"
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-shortcuts__label"
  }, it.label), /*#__PURE__*/React.createElement("span", {
    className: "hm-shortcuts__keys"
  }, (it.keys || []).map((k, i) => /*#__PURE__*/React.createElement(__ds_scope.Kbd, {
    key: i
  }, k)))))))), /*#__PURE__*/React.createElement("div", {
    className: "hm-shortcuts__foot"
  }, /*#__PURE__*/React.createElement("span", null, "Press ", /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "?"), " anywhere to reopen this"), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Kbd, null, "Esc"), " close"))));
}

/** The reserved, site-wide key map. Sections extend it; they never override it. */
const RESERVED_SHORTCUTS = [{
  name: "Global",
  items: [{
    label: "Open command palette",
    keys: ["Ctrl", "K"]
  }, {
    label: "Show keyboard shortcuts",
    keys: ["?"]
  }, {
    label: "Focus search on this page",
    keys: ["/"]
  }, {
    label: "Close overlay, blur field",
    keys: ["Esc"]
  }]
}, {
  name: "In an overlay",
  items: [{
    label: "Move selection",
    keys: ["\u2191", "\u2193"]
  }, {
    label: "Run selection",
    keys: ["\u21B5"]
  }]
}, {
  name: "In a list",
  items: [{
    label: "Previous / next item",
    keys: ["K", "J"]
  }, {
    label: "Open focused item",
    keys: ["\u21B5"]
  }]
}];
Object.assign(__ds_scope, { ShortcutsOverlay, RESERVED_SHORTCUTS });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/ShortcutsOverlay.jsx", error: String((e && e.message) || e) }); }

// components/navigation/Tabs.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Section switcher. Underline by default; pills for in-card filters. */
function Tabs({
  items = [],
  value,
  onChange,
  variant = "underline",
  className = "",
  ...rest
}) {
  const cls = ["hm-tabs", variant === "pills" ? "hm-tabs--pills" : "", className].filter(Boolean).join(" ");
  return /*#__PURE__*/React.createElement("div", _extends({
    className: cls,
    role: "tablist"
  }, rest), items.map(it => {
    const id = typeof it === "string" ? it : it.id;
    const label = typeof it === "string" ? it : it.label;
    const icon = typeof it === "string" ? null : it.icon;
    const count = typeof it === "string" ? null : it.count;
    return /*#__PURE__*/React.createElement("button", {
      key: id,
      type: "button",
      role: "tab",
      "aria-selected": value === id,
      className: "hm-tab" + (value === id ? " is-active" : ""),
      onClick: () => onChange && onChange(id)
    }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: icon,
      size: 14
    }) : null, label, count != null ? /*#__PURE__*/React.createElement("span", {
      style: {
        color: "var(--text-faint)",
        font: "var(--type-label)"
      }
    }, count) : null);
  }));
}
Object.assign(__ds_scope, { Tabs });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/Tabs.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/ArtScreen.jsx
try { (() => {
const NS = typeof window !== "undefined" && window.HampterDesignSystem_03c259 || {};
const {
  PostCard,
  Button,
  Badge,
  Card,
  Input,
  Textarea,
  Icon
} = NS;

/** GET /artportfolio — feed + admin composer (templates/artportfolio/feed.html) */
function ArtScreen() {
  const [composer, setComposer] = React.useState(false);
  return /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement(PageHead, {
    eyebrow: "128 posts \xB7 newest first",
    title: "Drawing Portfolio",
    meta: "A feed of drawings and sketches. Uploads are admin-only; everything else is public.",
    action: /*#__PURE__*/React.createElement(Button, {
      variant: composer ? "secondary" : "primary",
      icon: composer ? "x" : "plus",
      onClick: () => setComposer(v => !v)
    }, composer ? "Cancel" : "New post")
  }), /*#__PURE__*/React.createElement(Page, null, composer ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginBottom: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement(Card, {
    title: "New post",
    variant: "accent",
    footer: /*#__PURE__*/React.createElement("span", null, "jpeg \xB7 png \xB7 webp \xB7 35 MB max")
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-2)",
      marginBottom: "var(--space-2)"
    }
  }, /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    variant: "secondary"
  }, "Single image"), /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    variant: "ghost",
    disabled: true
  }, "Gallery"), /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    variant: "ghost",
    disabled: true
  }, "Board")), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      justifyContent: "center",
      gap: "var(--space-4)",
      padding: "var(--space-11)",
      border: "1px dashed var(--border-strong)",
      borderRadius: "var(--radius-md)",
      background: "var(--surface-inset)",
      color: "var(--text-muted)"
    }
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "image-plus",
    size: 22
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-body-sm)"
    }
  }, "Click to browse, or drop a file")), /*#__PURE__*/React.createElement(Textarea, {
    placeholder: "Caption (optional)",
    rows: 2
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-4)"
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    icon: "upload"
  }, "Upload"), /*#__PURE__*/React.createElement(Button, {
    variant: "ghost",
    onClick: () => setComposer(false)
  }, "Cancel")))) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      columns: 3,
      columnGap: "var(--space-6)"
    }
  }, POSTS.map((p, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    style: {
      breakInside: "avoid",
      marginBottom: "var(--space-6)"
    }
  }, /*#__PURE__*/React.createElement(PostCard, {
    caption: p.caption,
    date: p.date,
    badge: i === 0 ? /*#__PURE__*/React.createElement(Badge, {
      tone: "accent"
    }, "new") : null
  })))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      justifyContent: "center",
      padding: "var(--space-9) 0"
    }
  }, /*#__PURE__*/React.createElement(Button, {
    icon: "chevron-down"
  }, "Load more"))));
}
window.ArtScreen = ArtScreen;
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/ArtScreen.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/FitnessScreen.jsx
try { (() => {
const NS = typeof window !== "undefined" && window.HampterDesignSystem_03c259 || {};
const {
  CalorieRing,
  MacroRail,
  Card,
  Button,
  Badge,
  IconButton,
  Tabs,
  Icon,
  Kbd
} = NS;
const DAYS = ["S", "M", "T", "W", "T", "F", "S"];

/** GET /fitness — Today: ring, macro rails, week strip, meal slots, action bar. */
function FitnessScreen() {
  const [day, setDay] = React.useState(4);
  const total = MEALS.flatMap(m => m.items).reduce((s, i) => s + i.kcal, 0);
  return /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement(PageHead, {
    eyebrow: "Today \xB7 Sat 01 Aug",
    title: "Fitness",
    action: /*#__PURE__*/React.createElement(Button, {
      icon: "calendar-days"
    }, "Week")
  }), /*#__PURE__*/React.createElement(Page, null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "360px 1fr",
      gap: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-6)"
    }
  }, /*#__PURE__*/React.createElement(Card, null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement(CalorieRing, {
    value: total,
    target: 2100
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-5)"
    }
  }, /*#__PURE__*/React.createElement(MacroRail, {
    label: "protein",
    value: 128,
    target: 160,
    color: "var(--status-success)"
  }), /*#__PURE__*/React.createElement(MacroRail, {
    label: "carbs",
    value: 196,
    target: 240,
    color: "var(--accent)"
  }), /*#__PURE__*/React.createElement(MacroRail, {
    label: "fat",
    value: 54,
    target: 70,
    color: "var(--status-warning)"
  }))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-2)",
      marginTop: "var(--space-5)"
    }
  }, DAYS.map((d, i) => /*#__PURE__*/React.createElement("button", {
    key: i,
    onClick: () => setDay(i),
    style: {
      flex: 1,
      height: 44,
      borderRadius: "var(--radius-sm)",
      cursor: "pointer",
      border: "1px solid " + (i === day ? "var(--border-accent)" : "var(--border-subtle)"),
      background: i === day ? "var(--accent-tint)" : "var(--surface-inset)",
      color: i === day ? "var(--text-accent)" : "var(--text-muted)",
      font: "var(--type-label)"
    }
  }, d)))), /*#__PURE__*/React.createElement(Card, {
    title: "Streak",
    variant: "inset",
    footer: /*#__PURE__*/React.createElement("span", null, "longest 21 days")
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "baseline",
      gap: "var(--space-4)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "800 var(--text-40)/1 var(--font-display)",
      color: "var(--accent-warm)"
    }
  }, "9"), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-body-sm)",
      color: "var(--text-muted)"
    }
  }, "days logged in a row")))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-6)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)"
    }
  }, /*#__PURE__*/React.createElement(Tabs, {
    variant: "pills",
    value: "today",
    onChange: () => {},
    items: [{
      id: "today",
      label: "Today"
    }, {
      id: "recent",
      label: "Recent"
    }, {
      id: "fav",
      label: "Favourites"
    }, {
      id: "meals",
      label: "Meals"
    }]
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      gap: "var(--space-3)"
    }
  }, /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    icon: "scan-line"
  }, "Scan"), /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    icon: "search",
    shortcut: "/"
  }, "Search"), /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    variant: "ghost",
    icon: "copy"
  }, "Copy yesterday"))), MEALS.map(m => /*#__PURE__*/React.createElement(Card, {
    key: m.slot,
    title: m.slot,
    actions: /*#__PURE__*/React.createElement(IconButton, {
      icon: "plus",
      label: "Add to " + m.slot,
      size: "sm",
      variant: "accent"
    }),
    footer: /*#__PURE__*/React.createElement("span", null, m.items.reduce((s, i) => s + i.kcal, 0), " kcal")
  }, m.items.map(i => /*#__PURE__*/React.createElement("div", {
    key: i.name,
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-5)",
      padding: "var(--space-3) 0",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-body)",
      color: "var(--text-body)"
    }
  }, i.name), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      font: "var(--type-label)",
      color: "var(--text-faint)"
    }
  }, i.g, " g"), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-mono)",
      fontSize: "var(--text-13)",
      color: "var(--text-strong)",
      width: 56,
      textAlign: "right"
    }
  }, i.kcal), /*#__PURE__*/React.createElement(IconButton, {
    icon: "pencil",
    label: "Edit",
    size: "sm"
  }))))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)",
      font: "var(--type-label)",
      color: "var(--text-faint)"
    }
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "keyboard",
    size: 14
  }), " quick add: type a food and press ", /*#__PURE__*/React.createElement(Kbd, null, "\u21B5"), " to log its default portion")))));
}
window.FitnessScreen = FitnessScreen;
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/FitnessScreen.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/HubScreen.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const NS = typeof window !== "undefined" && window.HampterDesignSystem_03c259 || {};
const {
  HubCard,
  KbdGroup,
  Button
} = NS;

/** GET / — templates/hub/hub.html */
function HubScreen({
  onRoute,
  onOpenPalette
}) {
  return /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    className: "hm-grid-bg",
    style: {
      borderBottom: "1px solid var(--border-subtle)",
      padding: "var(--space-13) var(--space-8) var(--space-11)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: "var(--page-max)",
      margin: "0 auto"
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-eyebrow"
  }, "one rust binary \xB7 four side projects"), /*#__PURE__*/React.createElement("h1", {
    style: {
      font: "var(--type-display)",
      marginTop: "var(--space-6)",
      maxWidth: "12ch"
    }
  }, "Portfolio", /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--accent)"
    }
  }, ".")), /*#__PURE__*/React.createElement("p", {
    style: {
      marginTop: "var(--space-6)",
      font: "var(--type-body)",
      fontSize: "var(--text-18)",
      color: "var(--text-muted)",
      maxWidth: "52ch"
    }
  }, "A collection of personal projects \u2014 art, code, and whatever comes next."), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-5)",
      marginTop: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    size: "lg",
    icon: "image",
    onClick: () => onRoute("art")
  }, "See the drawings"), /*#__PURE__*/React.createElement("button", {
    onClick: onOpenPalette,
    style: {
      background: "none",
      border: "none",
      cursor: "pointer"
    }
  }, /*#__PURE__*/React.createElement(KbdGroup, {
    keys: ["Ctrl", "K"],
    hint: "everything else"
  }))))), /*#__PURE__*/React.createElement(Page, null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "repeat(2,1fr)",
      gap: "var(--space-6)"
    }
  }, SECTIONS.map(s => /*#__PURE__*/React.createElement(HubCard, _extends({
    key: s.id
  }, s, {
    onClick: () => onRoute(s.id === "drinks" ? "hub" : s.id)
  }))))));
}
window.HubScreen = HubScreen;
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/HubScreen.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/SiteChrome.jsx
try { (() => {
const NS = typeof window !== "undefined" && window.HampterDesignSystem_03c259 || {};
const {
  Icon,
  IconButton,
  Kbd,
  KbdGroup,
  Tooltip
} = NS;
function Wordmark({
  size = 17
}) {
  return /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-display)",
      fontWeight: 900,
      fontSize: size,
      letterSpacing: "-0.035em",
      color: "var(--text-strong)"
    }
  }, "hampter", /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--accent)"
    }
  }, "."));
}

/** Header from templates/base.html, restyled: wordmark, three nav links, Ctrl+K hint. */
function SiteHeader({
  route,
  onRoute,
  onOpenPalette
}) {
  const links = [{
    id: "art",
    label: "Drawing Portfolio"
  }, {
    id: "tasks",
    label: "Drawing Tasks"
  }, {
    id: "fitness",
    label: "Fitness"
  }];
  return /*#__PURE__*/React.createElement("header", {
    style: {
      position: "sticky",
      top: 0,
      zIndex: 40,
      height: "var(--header-height)",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      padding: "0 var(--space-8)",
      background: "rgba(14,12,20,.82)",
      backdropFilter: "blur(10px)",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement("a", {
    href: "#",
    onClick: e => {
      e.preventDefault();
      onRoute("hub");
    }
  }, /*#__PURE__*/React.createElement(Wordmark, null)), /*#__PURE__*/React.createElement("nav", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)"
    }
  }, links.map(l => /*#__PURE__*/React.createElement("button", {
    key: l.id,
    onClick: () => onRoute(l.id),
    className: "hm-tab" + (route === l.id ? " is-active" : ""),
    style: {
      height: 32
    }
  }, l.label))), /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)"
    }
  }, /*#__PURE__*/React.createElement("button", {
    onClick: onOpenPalette,
    className: "hm-input-wrap hm-input-wrap--sm",
    style: {
      width: 236,
      cursor: "pointer",
      background: "var(--ink-900)"
    }
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "search",
    size: 13
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      flex: 1,
      textAlign: "left",
      font: "var(--type-body-sm)",
      color: "var(--text-faint)"
    }
  }, "Search commands\u2026"), /*#__PURE__*/React.createElement(Kbd, null, "Ctrl"), /*#__PURE__*/React.createElement(Kbd, null, "K")), /*#__PURE__*/React.createElement(Tooltip, {
    label: "Keyboard shortcuts",
    shortcut: "?"
  }, /*#__PURE__*/React.createElement(IconButton, {
    icon: "keyboard",
    label: "Shortcuts",
    onClick: () => window.dispatchEvent(new KeyboardEvent("keydown", {
      key: "?"
    }))
  })), /*#__PURE__*/React.createElement(Tooltip, {
    label: "Admin panel"
  }, /*#__PURE__*/React.createElement(IconButton, {
    icon: "settings",
    label: "Admin"
  }))));
}
function SiteFooter() {
  return /*#__PURE__*/React.createElement("footer", {
    style: {
      borderTop: "1px solid var(--border-subtle)",
      marginTop: "var(--space-13)",
      padding: "var(--space-9) var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: "var(--page-max)",
      margin: "0 auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      font: "var(--type-label)",
      color: "var(--text-faint)"
    }
  }, /*#__PURE__*/React.createElement(Wordmark, {
    size: 14
  }), /*#__PURE__*/React.createElement("span", null, "axum \xB7 sqlite \xB7 htmx \xB7 no analytics, no cookies")));
}
function PageHead({
  eyebrow,
  title,
  meta,
  action,
  children
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "hm-grid-bg",
    style: {
      borderBottom: "1px solid var(--border-subtle)",
      padding: "var(--space-11) var(--space-8) var(--space-9)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: "var(--page-max)",
      margin: "0 auto",
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-5)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "flex-end",
      gap: "var(--space-6)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-4)"
    }
  }, eyebrow ? /*#__PURE__*/React.createElement("span", {
    className: "hm-eyebrow"
  }, eyebrow) : null, /*#__PURE__*/React.createElement("h1", {
    style: {
      font: "var(--type-h1)"
    }
  }, title)), action ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, action) : null), meta ? /*#__PURE__*/React.createElement("p", {
    style: {
      font: "var(--type-body)",
      color: "var(--text-muted)",
      maxWidth: "var(--prose-max)"
    }
  }, meta) : null, children));
}
function Page({
  children
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: "var(--page-max)",
      margin: "0 auto",
      padding: "var(--space-9) var(--space-8) 0"
    }
  }, children);
}
Object.assign(window, {
  Wordmark,
  SiteHeader,
  SiteFooter,
  PageHead,
  Page
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/SiteChrome.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/TasksScreen.jsx
try { (() => {
const NS = typeof window !== "undefined" && window.HampterDesignSystem_03c259 || {};
const {
  Tag,
  Badge,
  Select,
  Checkbox,
  Card,
  Button,
  Icon
} = NS;

/** GET /tasks — filter bar + task board (templates/tasks/feed.html) */
function TasksScreen() {
  const [subject, setSubject] = React.useState(null);
  const shown = TASKS.filter(t => !subject || t.subject === subject);
  const subjects = [...new Set(TASKS.map(t => t.subject))];
  return /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement(PageHead, {
    eyebrow: "42 tasks \xB7 18 done",
    title: "Drawing Tasks",
    meta: "Practice prompts attached to reference images \u2014 filter by subject, difficulty and task type, then grind."
  }), /*#__PURE__*/React.createElement(Page, null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "var(--rail-width) 1fr",
      gap: "var(--space-9)"
    }
  }, /*#__PURE__*/React.createElement("aside", {
    style: {
      position: "sticky",
      top: 80,
      alignSelf: "start",
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-7)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-4)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "hm-eyebrow"
  }, "Subject"), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexWrap: "wrap",
      gap: "var(--space-3)"
    }
  }, subjects.map(s => /*#__PURE__*/React.createElement(Tag, {
    key: s,
    interactive: true,
    selected: subject === s,
    onClick: () => setSubject(subject === s ? null : s)
  }, s.toLowerCase())))), /*#__PURE__*/React.createElement(Select, {
    label: "Difficulty",
    options: ["Any", "Easy", "Medium", "Hard"]
  }), /*#__PURE__*/React.createElement(Select, {
    label: "Task type",
    options: ["Any", "Timed", "Construction", "Study"]
  }), /*#__PURE__*/React.createElement(Checkbox, {
    label: "Hide completed",
    description: "18 tasks are marked done."
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-4)"
    }
  }, shown.map(t => /*#__PURE__*/React.createElement("div", {
    key: t.title,
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-6)",
      padding: "var(--space-5) var(--space-6)",
      background: "var(--surface-card)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-md)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 72,
      height: 52,
      flex: "none",
      borderRadius: "var(--radius-sm)",
      background: "var(--ink-900)",
      backgroundImage: "var(--texture-grid)",
      backgroundSize: "16px 16px",
      border: "1px solid var(--border-subtle)"
    }
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-3)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-h3)",
      fontSize: "var(--text-16)",
      color: "var(--text-strong)"
    }
  }, t.title), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-3)"
    }
  }, /*#__PURE__*/React.createElement(Tag, null, t.subject.toLowerCase()), /*#__PURE__*/React.createElement(Tag, null, t.type.toLowerCase()), /*#__PURE__*/React.createElement(Badge, {
    tone: t.difficulty === "Hard" ? "danger" : t.difficulty === "Medium" ? "warning" : "success"
  }, t.difficulty))), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-5)"
    }
  }, t.done ? /*#__PURE__*/React.createElement(Badge, {
    tone: "success",
    dot: true
  }, "done") : /*#__PURE__*/React.createElement(Button, {
    size: "sm",
    icon: "play"
  }, "Start"), /*#__PURE__*/React.createElement(Icon, {
    name: "chevron-right",
    size: 16
  }))))))));
}
window.TasksScreen = TasksScreen;
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/TasksScreen.jsx", error: String((e && e.message) || e) }); }

// ui_kits/portfolio/data.js
try { (() => {
const HM = window.HampterDesignSystem_03c259;

/* Sections mirror the real routes in Hampternt/drawingportfolio. */
const SECTIONS = [{
  id: "art",
  icon: "image",
  title: "Drawing Portfolio",
  description: "A feed of drawings and sketches.",
  meta: "128 posts",
  href: "/artportfolio"
}, {
  id: "tasks",
  icon: "list-checks",
  title: "Drawing Tasks",
  description: "Practice prompts on reference images — sorted by subject, difficulty, and task type.",
  meta: "42 tasks",
  href: "/tasks"
}, {
  id: "drinks",
  icon: "beer",
  title: "Drinks",
  description: "Party night drink tracker — join with a room code.",
  meta: "ring of fire · 3 man",
  href: "/drinks"
}, {
  id: "fitness",
  icon: "activity",
  title: "Fitness Tracker",
  description: "Track daily meals, calories, and macros.",
  meta: "session-gated",
  href: "/fitness"
}];
const POSTS = [{
  caption: "30 min gesture study — figure drawing warmup",
  date: "2026-07-21",
  h: 260
}, {
  caption: "Hands, again. Still not right.",
  date: "2026-07-18",
  h: 200
}, {
  caption: "Colour pass on the hamster knight",
  date: "2026-07-12",
  h: 300
}, {
  caption: "Perspective drill — one point interiors",
  date: "2026-07-04",
  h: 220
}];
const TASKS = [{
  title: "Gesture: 30s x 20",
  subject: "Figure",
  difficulty: "Easy",
  type: "Timed",
  done: true
}, {
  title: "Construct a head from the Loomis ball",
  subject: "Portrait",
  difficulty: "Medium",
  type: "Construction",
  done: true
}, {
  title: "Hands from reference — 6 angles",
  subject: "Anatomy",
  difficulty: "Hard",
  type: "Study",
  done: false
}, {
  title: "One-point interior with furniture",
  subject: "Perspective",
  difficulty: "Medium",
  type: "Construction",
  done: false
}, {
  title: "Value block-in, 5 values only",
  subject: "Rendering",
  difficulty: "Medium",
  type: "Timed",
  done: false
}];
const MEALS = [{
  slot: "breakfast",
  items: [{
    name: "Skyr, plain",
    g: 250,
    kcal: 160
  }, {
    name: "Blueberries",
    g: 80,
    kcal: 46
  }]
}, {
  slot: "lunch",
  items: [{
    name: "Chicken thigh, cooked",
    g: 190,
    kcal: 340
  }, {
    name: "Basmati rice",
    g: 210,
    kcal: 272
  }]
}, {
  slot: "dinner",
  items: [{
    name: "Salmon fillet",
    g: 160,
    kcal: 330
  }]
}, {
  slot: "snack",
  items: [{
    name: "Whey shake",
    g: 30,
    kcal: 118
  }]
}];

/* Mirrors the COMMANDS array in static/palette.js. */
const COMMANDS = [{
  id: "upload",
  group: "Admin",
  label: "Upload new drawing",
  icon: "upload",
  hint: "admin",
  adminOnly: true,
  keywords: "upload post new image add"
}, {
  id: "art",
  group: "Navigate",
  label: "Go to Art Portfolio",
  icon: "image",
  keywords: "feed gallery art drawings portfolio"
}, {
  id: "tasks",
  group: "Navigate",
  label: "Go to Drawing Tasks",
  icon: "list-checks",
  keywords: "tasks practice prompts drills study exercises"
}, {
  id: "drinks",
  group: "Navigate",
  label: "Go to Drinking Game",
  icon: "beer",
  keywords: "drinks drinking party game shots room"
}, {
  id: "fitness",
  group: "Navigate",
  label: "Go to Fitness Tracker",
  icon: "activity",
  keywords: "fitness food nutrition calories meals health tracker"
}, {
  id: "week",
  group: "Navigate",
  label: "Go to Fitness Week",
  icon: "calendar-days",
  keywords: "week trends weight streak fitness"
}, {
  id: "hub",
  group: "Navigate",
  label: "Go to Hub",
  icon: "home",
  keywords: "home hub index start main"
}, {
  id: "admin",
  group: "Admin",
  label: "Admin panel",
  icon: "settings",
  hint: "admin",
  adminOnly: true,
  keywords: "admin settings manage"
}];
Object.assign(window, {
  HM,
  SECTIONS,
  POSTS,
  TASKS,
  MEALS,
  COMMANDS
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/portfolio/data.js", error: String((e && e.message) || e) }); }

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.Button = __ds_scope.Button;

__ds_ns.Card = __ds_scope.Card;

__ds_ns.HubCard = __ds_scope.HubCard;

__ds_ns.Icon = __ds_scope.Icon;

__ds_ns.IconButton = __ds_scope.IconButton;

__ds_ns.Kbd = __ds_scope.Kbd;

__ds_ns.KbdGroup = __ds_scope.KbdGroup;

__ds_ns.PostCard = __ds_scope.PostCard;

__ds_ns.Tag = __ds_scope.Tag;

__ds_ns.CalorieRing = __ds_scope.CalorieRing;

__ds_ns.MacroRail = __ds_scope.MacroRail;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Toast = __ds_scope.Toast;

__ds_ns.ToastStack = __ds_scope.ToastStack;

__ds_ns.Tooltip = __ds_scope.Tooltip;

__ds_ns.Checkbox = __ds_scope.Checkbox;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Textarea = __ds_scope.Textarea;

__ds_ns.Radio = __ds_scope.Radio;

__ds_ns.Select = __ds_scope.Select;

__ds_ns.Switch = __ds_scope.Switch;

__ds_ns.CommandPalette = __ds_scope.CommandPalette;

__ds_ns.ShortcutsOverlay = __ds_scope.ShortcutsOverlay;

__ds_ns.RESERVED_SHORTCUTS = __ds_scope.RESERVED_SHORTCUTS;

__ds_ns.Tabs = __ds_scope.Tabs;

})();
