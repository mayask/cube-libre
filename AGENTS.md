# Contributor guidance

- The local host checkout is the source of truth. Edit and commit here;
  a virtual machine is only a disposable build/manual-test mirror.
- Sync host -> VM, excluding .git, build artifacts and private local configuration.
- Keep build/test output visible in the same guest desktop the tester is viewing.
  Do not run manual UI tests in a hidden second graphical session.
- Machine-specific paths, SSH details and display numbers belong in the ignored
  `.dev/vm.env` file, never in tracked files or published Git history.
- Ask before Bluetooth passthrough; assigning a host adapter to a VM interrupts
  Bluetooth on the host. Coordinate physical-cube wakeup with the tester.
- Never send cube reset, calibration, pairing, or firmware commands.
- Unknown/stale cube states must not be labelled live. Recover from packet gaps
  by requesting authoritative state; don't apply a move to a knowingly stale cube.
- Keep protocol/maths separate from BLE and Dioxus. No telemetry or cloud.
- Desktop is the initial supported build. Mobile and web are future integration
  targets, not claims of completed support.
