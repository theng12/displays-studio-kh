# ddc-macos 0.2.2 (vendored)

Source: the published `ddc-macos` 0.2.2 Cargo package (MIT; see `LICENSE`).

Local differences in the Apple Silicon path: release owned Core Foundation and IOKit objects at the end of each scan/action, and reject ambiguous proxy discovery instead of crossing the next framebuffer boundary. The framebuffer boundary follows the registry ordering used in MonitorControl's MIT-licensed `Arm64DDC.swift`. Re-evaluate these changes before upgrading the dependency.
