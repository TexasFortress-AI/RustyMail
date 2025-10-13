module.exports = {
  apps: [
    {
      name: 'rustymail-backend',
      script: './target/release/rustymail-server',
      cwd: '/Users/au/src/RustyMail',
      instances: 1,
      autorestart: true,
      watch: false,
      max_memory_restart: '500M',
      env: {
        NODE_ENV: 'production'
      },
      error_file: './logs/backend-error.log',
      out_file: './logs/backend-out.log',
      log_date_format: 'YYYY-MM-DD HH:mm:ss Z',
      merge_logs: true
    },
    {
      name: 'rustymail-frontend',
      script: 'npm',
      args: 'run dev',
      cwd: '/Users/au/src/RustyMail/frontend/rustymail-app-main',
      instances: 1,
      autorestart: true,
      watch: false,
      max_memory_restart: '300M',
      error_file: '../../logs/frontend-error.log',
      out_file: '../../logs/frontend-out.log',
      log_date_format: 'YYYY-MM-DD HH:mm:ss Z',
      merge_logs: true
    }
  ]
};
