module.exports = {
  apps: [
    {
      name: 'rustymail-backend',
      script: './target/release/rustymail-server',
      cwd: '/Users/au/src/RustyMail',
      env: {
        RUST_LOG: 'debug',
      },
      out_file: './logs/backend-out.log',
      error_file: './logs/backend-error.log',
      time: true,
      autorestart: true,
      max_restarts: 10,
      min_uptime: '10s',
    },
    {
      name: 'rustymail-frontend',
      script: 'npm',
      args: 'run dev',
      cwd: '/Users/au/src/RustyMail/frontend/rustymail-app-main',
      out_file: '../../logs/frontend-out.log',
      error_file: '../../logs/frontend-error.log',
      time: true,
      autorestart: true,
      max_restarts: 10,
      min_uptime: '10s',
    },
  ],
};
