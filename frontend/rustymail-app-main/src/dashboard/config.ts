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