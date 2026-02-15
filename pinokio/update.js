module.exports = {
  run: [
    // Pull latest changes
    {
      method: "shell.run",
      params: {
        path: "app",
        message: ["git pull"]
      }
    },
    // Rebuild backend
    {
      method: "shell.run",
      params: {
        path: "app",
        message: ["cargo build --release"],
        env: { CARGO_TERM_COLOR: "always" }
      }
    },
    // Rebuild frontend
    {
      method: "shell.run",
      params: {
        path: "app/frontend/rustymail-app-main",
        message: ["npm install", "npm run build"]
      }
    },
    {
      method: "shell.run",
      params: {
        message: ["echo 'Update complete! Click Start to launch the updated version.'"]
      }
    }
  ]
}
