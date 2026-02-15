module.exports = {
  version: "5.0",
  title: "RustyMail",
  description: "Local IMAP email gateway with HTTP REST API, MCP interface, and WebUI for AI agents.",
  icon: "icon.png",
  pre: [
    {
      icon: "fa-brands fa-rust",
      title: "Rust Toolchain",
      description: "Rust and cargo must be installed. Visit rustup.rs to install.",
      href: "https://rustup.rs/"
    },
    {
      icon: "fa-brands fa-node-js",
      title: "Node.js",
      description: "Node.js 18+ is required for the frontend build.",
      href: "https://nodejs.org/"
    }
  ],
  menu: async (kernel, info) => {
    let installed = info.exists("app/target/release/rustymail-server")
    let running = {
      install: info.running("install.js"),
      start: info.running("start.js"),
      update: info.running("update.js"),
    }

    if (running.install) {
      return [
        { default: true, icon: "fa-solid fa-plug", text: "Installing...", href: "install.js" }
      ]
    }

    if (running.update) {
      return [
        { default: true, icon: "fa-solid fa-arrows-rotate", text: "Updating...", href: "update.js" }
      ]
    }

    if (installed) {
      if (running.start) {
        let local = info.local("start.js")
        if (local && local.url) {
          return [
            { default: true, icon: "fa-solid fa-rocket", text: "Open WebUI", href: local.url },
            { icon: "fa-solid fa-terminal", text: "Terminal", href: "start.js" },
            { icon: "fa-solid fa-arrows-rotate", text: "Update", href: "update.js" },
            { icon: "fa-regular fa-circle-xmark", text: "Reset", href: "reset.js", confirm: "This will delete the app and all data. Are you sure?" },
          ]
        } else {
          return [
            { default: true, icon: "fa-solid fa-terminal", text: "Starting...", href: "start.js" }
          ]
        }
      } else {
        return [
          { default: true, icon: "fa-solid fa-power-off", text: "Start", href: "start.js" },
          { icon: "fa-solid fa-arrows-rotate", text: "Update", href: "update.js" },
          { icon: "fa-solid fa-plug", text: "Reinstall", href: "install.js", confirm: "This will rebuild from scratch. Continue?" },
          { icon: "fa-regular fa-circle-xmark", text: "Reset", href: "reset.js", confirm: "This will delete the app and all data. Are you sure?" },
        ]
      }
    } else {
      return [
        { default: true, icon: "fa-solid fa-plug", text: "Install", href: "install.js" }
      ]
    }
  }
}
