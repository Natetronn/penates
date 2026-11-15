# Security

Penates wraps sites you sign in to, so security reports are welcome.

## Reporting a problem
Please report security problems privately through GitHub:
**[Report a vulnerability](https://github.com/Natetronn/penates/security/advisories/new)**
(Security tab → Report a vulnerability). Do not open a public issue for them.

Include what you found, how to reproduce it, and the Penates version and OS. You will get a reply as
soon as possible; fixes are released as soon as they are ready and credited in the changelog unless you
prefer otherwise.

## Supported versions
Only the latest release gets fixes. Penates updates itself (signed updates) on AppImage, `.deb`, `.rpm`,
Windows and macOS installs; AUR users update with their package manager.

## What is in scope
- A wrapped site reaching Penates, the manager or your files (wrapped sites get no Penates API).
- Link handling that opens local files or other programs, or floods the desktop.
- `.penates` imports or the app catalog doing more than they show (they may not add JavaScript, and imports
  switch off camera, microphone and developer tools).
- Anything that lets an update install without a valid signature.

Bugs in the sites themselves, and in WebKitGTK, WebView2 or WKWebView, belong with their makers.
