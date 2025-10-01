// Dashboard configuration
export const config = {
  // API configuration
  api: {
    baseUrl: import.meta.env.VITE_API_URL || 'http://localhost:9437/api',
    // Use environment variable or default test key
    apiKey: import.meta.env.VITE_RUSTYMAIL_API_KEY || 'test-rustymail-key-2024',
  },

  // Dashboard specific settings
  dashboard: {
    refreshInterval: 5000, // 5 seconds
    maxEmailsToShow: 20,
  }
};

export default config;