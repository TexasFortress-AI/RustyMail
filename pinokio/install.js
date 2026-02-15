module.exports = {
  run: [
    // Clone the RustyMail repository
    {
      method: "shell.run",
      params: {
        message: [
          "if [ ! -d app ]; then git clone https://github.com/TexasFortress-AI/RustyMail.git app; fi"
        ]
      }
    },
    // Build the Rust backend (release mode)
    {
      method: "shell.run",
      params: {
        path: "app",
        message: ["cargo build --release"],
        env: { CARGO_TERM_COLOR: "always" }
      }
    },
    // Install frontend dependencies and build
    {
      method: "shell.run",
      params: {
        path: "app/frontend/rustymail-app-main",
        message: ["npm install", "npm run build"]
      }
    },
    // Create .env from template and generate a random API key
    {
      method: "shell.run",
      params: {
        path: "app",
        message: [
          "if [ ! -f .env ]; then cp .env.example .env && API_KEY=$(openssl rand -hex 32) && sed -i '' \"s/your-secure-api-key-here/${API_KEY}/g\" .env && echo 'Created .env with generated API key'; else echo '.env already exists, skipping'; fi"
        ]
      }
    },
    // Create required data directories
    {
      method: "shell.run",
      params: {
        path: "app",
        message: [
          "mkdir -p data config logs",
          "echo 'Installation complete! Edit app/.env to add your IMAP credentials, then click Start.'"
        ]
      }
    }
  ]
}
