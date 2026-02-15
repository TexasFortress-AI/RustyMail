module.exports = {
  daemon: true,
  run: [
    // Start the Rust backend server
    {
      method: "shell.run",
      params: {
        path: "app",
        message: ["./target/release/rustymail-server"],
        env: { RUST_LOG: "info" },
        on: [{ event: "/Starting HTTP server/", done: true }]
      }
    },
    // Start the frontend Vite dev server
    {
      method: "shell.run",
      params: {
        path: "app/frontend/rustymail-app-main",
        message: ["npm run dev"],
        on: [{ event: "/(http:\\/\\/localhost:[0-9]+)/", done: true }]
      }
    },
    // Store the frontend URL for the "Open WebUI" button
    {
      method: "local.set",
      params: {
        url: "{{input.event[1]}}"
      }
    }
  ]
}
