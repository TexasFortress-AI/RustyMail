module.exports = {
  apps : [{
    name: 'rustymail-backend',
    script: './target/release/rustymail-server',
    cwd: '/Users/au/src/RustyMail',
  }, {
    name: 'rustymail-frontend',
    script: 'npm',
    args: 'run dev',
    cwd: '/Users/au/src/RustyMail/frontend/rustymail-app-main',
  }]
};
