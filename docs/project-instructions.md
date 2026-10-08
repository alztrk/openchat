# Project instructions

OpenChat can apply project-specific instructions to conversations attached to a project. Edit them from the project's options, or create `.openchat/instructions.md` in the project folder and write UTF-8 Markdown instructions there. OpenChat reads the file when a new response run starts and sends its contents with that run's provider request.

The file is optional. A missing or whitespace-only file adds no project instructions. The editor enforces a 16 KiB UTF-8 byte limit and saves through an atomic replacement. Invalid UTF-8, read failures, and paths that resolve outside the attached project stop the request with a clear error; OpenChat does not silently ignore a configured but invalid file.

The text is saved in the run checkpoint so a resumed run keeps the instructions it started with even if the file changes later. Start a new run to apply edits. Project instructions are sent to the selected provider as part of the prompt, so keep them free of secrets and data that should not leave the device.

Project instructions apply only within their project context. They do not replace the user's current request or OpenChat's higher-priority instructions. Project-scoped Skills and broader profile settings such as provider/model defaults are not implemented yet.
