// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import DOMPurify from 'dompurify';
import { API_BASE_URL } from '../../config/api';

export interface SanitizeEmailOptions {
  showImages: boolean;
  messageId?: string;
  accountId?: string;
}

/** Parse #rgb / #rrggbb / rgb() / named light colors into perceived luminance 0–1. */
function colorLuminance(raw: string): number | null {
  const s = raw.trim().toLowerCase();
  if (!s) return null;

  const namedLight = new Set([
    'white', 'snow', 'ivory', 'azure', 'ghostwhite', 'whitesmoke',
    'floralwhite', 'aliceblue', 'lavenderblush', 'mintcream', 'lightyellow',
    'lightcyan', 'lightgray', 'lightgrey', 'gainsboro', 'silver',
  ]);
  if (namedLight.has(s)) return 0.95;
  if (s === 'transparent' || s === 'inherit' || s === 'currentcolor') return null;

  let r = 0;
  let g = 0;
  let b = 0;

  const hex = s.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (hex) {
    const h = hex[1];
    if (h.length === 3) {
      r = parseInt(h[0] + h[0], 16);
      g = parseInt(h[1] + h[1], 16);
      b = parseInt(h[2] + h[2], 16);
    } else {
      r = parseInt(h.slice(0, 2), 16);
      g = parseInt(h.slice(2, 4), 16);
      b = parseInt(h.slice(4, 6), 16);
    }
  } else {
    const rgb = s.match(/^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/);
    if (!rgb) return null;
    r = Number(rgb[1]);
    g = Number(rgb[2]);
    b = Number(rgb[3]);
    if (r <= 1 && g <= 1 && b <= 1) {
      r *= 255;
      g *= 255;
      b *= 255;
    }
  }

  // Relative luminance (sRGB approx)
  const lin = (c: number) => {
    const x = c / 255;
    return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function isLightColor(raw: string): boolean {
  const lum = colorLuminance(raw);
  return lum != null && lum >= 0.75;
}

function isDarkColor(raw: string): boolean {
  const lum = colorLuminance(raw);
  return lum != null && lum <= 0.25;
}

/** Strip light text colors / dark backgrounds that break our light email canvas. */
function neutralizeContrastAttrs(node: Element): void {
  const colorAttr = node.getAttribute('color');
  if (colorAttr && isLightColor(colorAttr)) {
    node.removeAttribute('color');
  }

  const bgcolor = node.getAttribute('bgcolor');
  if (bgcolor && isDarkColor(bgcolor)) {
    node.removeAttribute('bgcolor');
  }

  const style = node.getAttribute('style');
  if (!style) return;

  // Drop color / background declarations that fight a light canvas
  const cleaned = style
    .split(';')
    .map((part) => part.trim())
    .filter((part) => {
      if (!part) return false;
      const m = part.match(/^(color|background(?:-color)?)\s*:\s*(.+)$/i);
      if (!m) return true;
      const prop = m[1].toLowerCase();
      const val = m[2].trim();
      if (prop === 'color' && isLightColor(val)) return false;
      if (prop.startsWith('background') && isDarkColor(val)) return false;
      return true;
    })
    .join('; ');

  if (cleaned) {
    node.setAttribute('style', cleaned);
  } else {
    node.removeAttribute('style');
  }
}

/**
 * Sanitize HTML email for safe inline rendering on a forced light canvas.
 * Rewrites cid: images when showImages is true; otherwise replaces with placeholders.
 */
export function sanitizeEmailHtml(html: string, options: SanitizeEmailOptions): string {
  const { showImages, messageId, accountId } = options;

  DOMPurify.removeAllHooks();

  if (!showImages) {
    DOMPurify.addHook('uponSanitizeElement', (node, data) => {
      if (data.tagName === 'img') {
        const placeholder = document.createElement('span');
        placeholder.className =
          'email-img-blocked inline-block px-2 py-1 text-xs rounded border';
        placeholder.textContent = '[Image blocked]';
        node.parentNode?.replaceChild(placeholder, node);
      }
    });
  } else {
    DOMPurify.addHook('afterSanitizeAttributes', (node) => {
      if (node.tagName === 'IMG') {
        const src = node.getAttribute('src') || '';
        if (src.startsWith('cid:')) {
          const contentId = src.substring(4);
          if (messageId && accountId) {
            const inlineUrl = `${API_BASE_URL}/dashboard/attachments/${encodeURIComponent(messageId)}/inline/${encodeURIComponent(contentId)}?account_id=${encodeURIComponent(accountId)}`;
            node.setAttribute('src', inlineUrl);
          } else {
            const placeholder = document.createElement('span');
            placeholder.className =
              'email-img-embedded inline-block px-2 py-1 text-xs rounded border';
            placeholder.textContent = '[Embedded image]';
            node.parentNode?.replaceChild(placeholder, node);
          }
        }
      }
    });
  }

  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    if (node.tagName === 'A') {
      node.setAttribute('target', '_blank');
      node.setAttribute('rel', 'noopener noreferrer');
    }
    if (node instanceof Element) {
      neutralizeContrastAttrs(node);
    }
  });

  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: [
      'p', 'br', 'strong', 'em', 'u', 's', 'a', 'ul', 'ol', 'li', 'blockquote',
      'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'table', 'thead',
      'tbody', 'tr', 'th', 'td', 'img', 'hr', 'div', 'span', 'font', 'center', 'b', 'i',
    ],
    ALLOWED_ATTR: showImages
      ? [
          'href', 'target', 'rel', 'src', 'alt', 'width', 'height', 'style', 'class',
          'align', 'valign', 'bgcolor', 'color', 'size', 'face', 'border',
          'cellpadding', 'cellspacing',
        ]
      : [
          'href', 'target', 'rel', 'class', 'align', 'valign', 'bgcolor', 'color',
          'size', 'face', 'border', 'cellpadding', 'cellspacing', 'style',
        ],
    ALLOW_DATA_ATTR: false,
    FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form', 'input', 'style'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick', 'onmouseover'],
  }) as unknown as string;
}
