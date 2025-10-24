// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Dashboard configuration
export const config = {
  // API configuration
  api: {
    baseUrl: import.meta.env.VITE_API_URL || '',
    // API key must be set via VITE_RUSTYMAIL_API_KEY environment variable
    apiKey: import.meta.env.VITE_RUSTYMAIL_API_KEY || '',
  },

  // Dashboard specific settings
  dashboard: {
    refreshInterval: 5000, // 5 seconds
    maxEmailsToShow: 20,
  }
};

export default config;