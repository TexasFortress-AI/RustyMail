// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Loads images whose URL needs the API key. An <img src> request cannot send
// X-API-Key, so sanitizeEmailHtml puts protected URLs in AUTH_SRC_ATTRIBUTE
// instead; this hook fetches each one with the key and shows it from an
// object URL.

import { useEffect, type RefObject } from 'react';
import { fetchObjectUrl } from '../api/authFetch';
import { AUTH_SRC_ATTRIBUTE } from '../utils/sanitizeEmailHtml';

/** Re-runs whenever `renderedHtml` changes; pass '' when nothing is rendered. */
export function useAuthenticatedImages(containerRef: RefObject<HTMLElement>, renderedHtml: string) {
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !renderedHtml) return;
    const objectUrls: string[] = [];
    let cancelled = false;
    container.querySelectorAll<HTMLImageElement>(`img[${AUTH_SRC_ATTRIBUTE}]`).forEach((image) => {
      const protectedUrl = image.getAttribute(AUTH_SRC_ATTRIBUTE) || '';
      fetchObjectUrl(protectedUrl)
        .then((objectUrl) => {
          if (cancelled) return URL.revokeObjectURL(objectUrl);
          objectUrls.push(objectUrl);
          image.src = objectUrl;
        })
        .catch((error) => console.error('Failed to load inline image:', error));
    });
    return () => {
      cancelled = true;
      objectUrls.forEach((objectUrl) => URL.revokeObjectURL(objectUrl));
    };
  }, [containerRef, renderedHtml]);
}
