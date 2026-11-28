---
name: anywhere-plugin-dev
description: Build and test an Anywhere API 2 plugin with the bundled SDK and CLI.
---

Read the project Manifest and the SDK README in vendor/plugin-sdk first. Use
the editor schema in vendor/plugin-sdk/schemas for the exact field names.

Run npm run dev for the browser mock or background sandbox. Use
npm run build, npm test, npm run validate and npm run pack to verify a change.
Review the generated archive in Anywhere before enabling the plugin.

Declare only the permissions the command needs. Automatic Context resolvers
use defineResolver and return data under plugin.<package-id>.*. Use the stable
command IDs from the Manifest. Put frontend development controls in the browser
mock entry so the archive contains only the production UI.
