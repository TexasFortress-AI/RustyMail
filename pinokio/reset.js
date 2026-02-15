module.exports = {
  run: [
    {
      method: "fs.rm",
      params: {
        path: "app"
      }
    },
    {
      method: "shell.run",
      params: {
        message: ["echo 'Reset complete. Click Install to set up RustyMail again.'"]
      }
    }
  ]
}
