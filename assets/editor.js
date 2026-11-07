// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Visual editor for the "Aktuelle Informationen" text.
//
// Without JavaScript, the form falls back to a plain HTML text field. The
// server sanitizes the result in any case. This script only produces markup
// that survives the sanitizer: CSS classes instead of inline styles, as the
// Content Security Policy forbids the latter.
"use strict";

(() => {
  const form = document.getElementById("news-form");
  const editor = document.getElementById("editor");
  const source = document.getElementById("editor-source");
  const textarea = document.getElementById("html");
  const toolbar = form && form.querySelector(".editor-toolbar");
  if (!form || !editor || !source || !textarea || !toolbar) {
    return;
  }

  // `execCommand` writes `<font color>`, which `normalize` turns into
  // `<span class="text-red">`. This color marks the spans to convert.
  const RED = "#ff0000";
  const RED_STYLE = /color:\s*(rgb\(255,\s*0,\s*0\)|#ff0000|red)\b/i;
  const ALIGNABLE = "p, h2, h3";
  const BLOCKS = "p, h2, h3, ul, ol, hr";
  let dirty = false;
  // The last selection inside the editor. Tapping a toolbar button can move
  // the focus and lose the selection, e.g., on touch devices, so commands
  // restore it first.
  let lastRange = null;

  // The textarea holds the HTML that the server already sanitized.
  editor.innerHTML = textarea.value;
  editor.hidden = false;
  toolbar.hidden = false;
  source.hidden = true;
  document.execCommand("defaultParagraphSeparator", false, "p");
  document.execCommand("styleWithCSS", false, false);

  /** Returns the current selection range if it's inside the editor. */
  function editorRange() {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) {
      return null;
    }
    const range = selection.getRangeAt(0);
    return editor.contains(range.commonAncestorContainer) ? range : null;
  }

  /** Restores a saved selection range, e.g., after a dialog took focus. */
  function restoreRange(range) {
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }

  /** Replaces `element` with a new element `tag` that has the same content. */
  function replaceTag(element, tag) {
    const replacement = document.createElement(tag);
    for (const name of element.classList) {
      replacement.classList.add(name);
    }
    while (element.firstChild) {
      replacement.appendChild(element.firstChild);
    }
    element.replaceWith(replacement);
    return replacement;
  }

  /** Replaces `element` with its children. */
  function unwrap(element) {
    element.replaceWith(...element.childNodes);
  }

  /** Wraps top-level text and inline elements into paragraphs. */
  function wrapLooseContent() {
    let paragraph = null;
    for (const node of Array.from(editor.childNodes)) {
      const isBlock = node.nodeType === Node.ELEMENT_NODE && node.matches(BLOCKS);
      const isBlank = node.nodeType === Node.TEXT_NODE && node.textContent.trim() === "";
      if (isBlock) {
        paragraph = null;
      } else if (!(isBlank && !paragraph)) {
        if (!paragraph) {
          paragraph = document.createElement("p");
          editor.insertBefore(paragraph, node);
        }
        paragraph.appendChild(node);
      }
    }
  }

  /** Converts browser-specific markup into what the sanitizer keeps. */
  function normalize() {
    for (const font of editor.querySelectorAll("font")) {
      if ((font.getAttribute("color") || "").toLowerCase() === RED) {
        replaceTag(font, "span").classList.add("text-red");
      } else {
        unwrap(font);
      }
    }
    for (const element of editor.querySelectorAll("[style]")) {
      if (element.tagName === "SPAN" && RED_STYLE.test(element.getAttribute("style"))) {
        element.classList.add("text-red");
      }
      element.removeAttribute("style");
    }
    for (const element of editor.querySelectorAll("b")) {
      replaceTag(element, "strong");
    }
    for (const element of editor.querySelectorAll("i")) {
      replaceTag(element, "em");
    }
    for (const element of editor.querySelectorAll("div")) {
      replaceTag(element, "p");
    }
    for (const element of editor.querySelectorAll("[class='']")) {
      element.removeAttribute("class");
    }
    wrapLooseContent();
  }

  /** Toggles red color on the selected text. */
  function toggleRed() {
    const range = editorRange();
    if (!range) {
      return;
    }
    const start = range.startContainer.nodeType === Node.ELEMENT_NODE
      ? range.startContainer
      : range.startContainer.parentElement;
    const red = start && start.closest(".text-red");
    if (red && editor.contains(red)) {
      unwrap(red);
    } else if (!range.collapsed) {
      document.execCommand("foreColor", false, RED);
      normalize();
    }
  }

  /** Toggles centering of the paragraphs in the selection. */
  function toggleCenter() {
    const range = editorRange();
    if (!range) {
      return;
    }
    wrapLooseContent();
    const blocks = Array.from(editor.children).filter(
      (child) => child.matches(ALIGNABLE) && range.intersectsNode(child),
    );
    const center = !blocks.every((block) => block.classList.contains("text-center"));
    for (const block of blocks) {
      block.classList.toggle("text-center", center);
    }
  }

  /** Turns the selected text into a link. */
  function insertLink() {
    const range = editorRange();
    if (!range || range.collapsed) {
      window.alert("Bitte zuerst den Text markieren, der verlinkt werden soll.");
      return;
    }
    const saved = range.cloneRange();
    const url = (window.prompt("Adresse des Links, z.B. https://www.example.de oder /kontakt/:", "https://") || "").trim();
    restoreRange(saved);
    if (!url || url === "https://") {
      return;
    }
    if (!/^(https?:\/\/|mailto:|tel:|\/)/i.test(url)) {
      window.alert("Nur Adressen mit https://, http://, mailto:, tel: oder / sind möglich.");
      return;
    }
    document.execCommand("createLink", false, url);
  }

  /** Removes all formatting from the selection, including centering. */
  function clearFormatting() {
    document.execCommand("removeFormat");
    document.execCommand("unlink");
    const range = editorRange();
    if (range) {
      for (const block of editor.querySelectorAll(".text-center")) {
        if (range.intersectsNode(block)) {
          block.classList.remove("text-center");
        }
      }
      for (const red of editor.querySelectorAll(".text-red")) {
        if (range.intersectsNode(red)) {
          unwrap(red);
        }
      }
    }
  }

  const commands = {
    bold: () => document.execCommand("bold"),
    red: toggleRed,
    center: toggleCenter,
    rule: () => document.execCommand("insertHorizontalRule"),
    list: () => document.execCommand("insertUnorderedList"),
    link: insertLink,
    clear: clearFormatting,
  };

  // The toolbar is a single tab stop, and the arrow keys move between its
  // buttons, as in the WAI-ARIA toolbar pattern:
  // https://www.w3.org/WAI/ARIA/apg/patterns/toolbar/
  const buttons = Array.from(toolbar.querySelectorAll("button"));
  function makeTabStop(button) {
    for (const other of buttons) {
      other.tabIndex = other === button ? 0 : -1;
    }
  }
  makeTabStop(buttons[0]);
  toolbar.addEventListener("focusin", (event) => {
    if (buttons.includes(event.target)) {
      makeTabStop(event.target);
    }
  });
  toolbar.addEventListener("keydown", (event) => {
    const index = buttons.indexOf(document.activeElement);
    const targets = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: buttons.length - 1 };
    if (index < 0 || !(event.key in targets)) {
      return;
    }
    event.preventDefault();
    buttons[(targets[event.key] + buttons.length) % buttons.length].focus();
  });

  // Tells screen readers whether the selection already has a formatting.
  const toggles = {
    bold: () => document.queryCommandState("bold"),
    red: (node) => Boolean(node.closest(".text-red")),
    center: (node) => Boolean(node.closest(".text-center")),
    list: () => document.queryCommandState("insertUnorderedList"),
  };
  function updatePressed() {
    const range = editorRange();
    if (!range) {
      return;
    }
    const container = range.startContainer;
    const node = container.nodeType === Node.ELEMENT_NODE ? container : container.parentElement;
    for (const [name, isActive] of Object.entries(toggles)) {
      const pressed = editor.contains(node) && isActive(node);
      toolbar.querySelector(`[data-command="${name}"]`).setAttribute("aria-pressed", String(pressed));
    }
  }
  for (const name of Object.keys(toggles)) {
    toolbar.querySelector(`[data-command="${name}"]`).setAttribute("aria-pressed", "false");
  }

  // Keeps the selection in the editor when a toolbar button is pressed.
  toolbar.addEventListener("mousedown", (event) => {
    if (event.target.closest("button")) {
      event.preventDefault();
    }
  });

  document.addEventListener("selectionchange", () => {
    const range = editorRange();
    if (range) {
      lastRange = range.cloneRange();
      updatePressed();
    }
  });

  toolbar.addEventListener("click", (event) => {
    const button = event.target.closest("button[data-command]");
    const command = button && commands[button.dataset.command];
    if (command) {
      // The current selection, or the last one if the tap lost it. Taken
      // before focus(), which may move the caret, and re-applied after it.
      const fallback = lastRange && editor.contains(lastRange.startContainer) ? lastRange : null;
      const range = editorRange() || fallback;
      editor.focus();
      if (range) {
        restoreRange(range);
      }
      command();
      updatePressed();
      dirty = true;
    }
  });

  // Pasted text from Word or websites brings foreign formatting. Only the
  // text is kept, the formatting is applied with the toolbar.
  editor.addEventListener("paste", (event) => {
    event.preventDefault();
    const text = event.clipboardData ? event.clipboardData.getData("text/plain") : "";
    document.execCommand("insertText", false, text);
  });

  editor.addEventListener("input", () => {
    dirty = true;
  });

  form.addEventListener("submit", () => {
    normalize();
    textarea.value = editor.innerHTML;
    dirty = false;
  });

  window.addEventListener("beforeunload", (event) => {
    if (dirty) {
      event.preventDefault();
    }
  });
})();
