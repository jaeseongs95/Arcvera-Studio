# Windows prerequisites

[한국어](RUNTIME.ko.md)

Arcvera-Studio packages do not include Microsoft runtime DLLs or installers.
The app and its installer do not download, install or update these prerequisites.
Obtain any missing prerequisite directly from Microsoft and install it yourself:

- **Desktop:** Microsoft Edge WebView2 Evergreen Runtime, required for the window.
  Use Microsoft's [WebView2 download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
  The Desktop installer stops before modifying the installation if WebView2 is
  missing or below a configured minimum version, including during updates. Install
  or update the runtime yourself, then rerun the Desktop installer. The portable
  edition needs the same prerequisite before launching.
- **Core voice helper:** if voice initialization reports a missing Visual C++
  runtime (for example `VCRUNTIME140.dll`), use Microsoft's
  [supported Visual C++ Redistributable instructions](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).
  Select the x64 package for the x64 Core helper and follow the version requirement
  in the selected Core package's `RUNTIME.md`. Do not copy DLLs from download sites
  or another installation. Core itself is installed separately.

Existing Windows-managed runtime installations remain under their provider's
update policy. No Windows runtime, existing user settings or authentication data
is removed by this distribution policy. Redistributable open-source components
and their license notices remain in the relevant packages.

A missing-runtime clean Windows installation, the full Desktop installer and
the Core voice feature on such a host have not been verified in this release's
local fixture tests. See the release notes for the tested boundaries.
