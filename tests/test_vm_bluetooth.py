"""Exercise the handoff helper with fake commands; never touch a real adapter."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET


class BluetoothHelperTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        (self.root / ".dev").mkdir()
        source = Path(__file__).resolve().parents[1] / "scripts/vm-bluetooth.sh"
        self.script = self.root / "scripts/vm-bluetooth.sh"
        shutil.copyfile(source, self.script)
        (self.root / ".dev/vm.env").write_text(
            "export VM_LIBVIRT_DOMAIN=test-vm\n"
            "export VM_LIBVIRT_URI=qemu:///system\n"
            "export VM_BLUETOOTH_VENDOR=abcd\n"
            "export VM_BLUETOOTH_PRODUCT=1234\n"
        )
        fake = self.root / "bin"
        fake.mkdir()
        virsh = fake / "virsh"
        virsh.write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
args = sys.argv[1:]
with open(os.environ["TEST_CALLS"], "a") as log:
    log.write(" ".join(args) + "\\n")
if "uri" in args: print(os.environ.get("TEST_URI", "qemu:///system"))
if "domstate" in args: print("running")
if "attach-device" in args or "detach-device" in args:
    Path(os.environ["TEST_XML"]).write_text(Path(args[args.index("--file") + 1]).read_text())
''')
        virsh.chmod(0o755)
        lsusb = fake / "lsusb"
        lsusb.write_text('''#!/usr/bin/env python3
import os
for i in range(int(os.environ.get("TEST_RADIOS", "1"))):
    print(f"Bus 999 Device {998 + i}: ID abcd:1234 Synthetic Bluetooth Radio")
''')
        lsusb.chmod(0o755)
        self.env = dict(os.environ, PATH=f"{fake}:{os.environ['PATH']}",
                        TEST_CALLS=str(self.root / "calls"), TEST_XML=str(self.root / "device.xml"))
        self.env.pop("TEST_URI", None)
        self.env.pop("TEST_RADIOS", None)

    def run_helper(self, *args):
        return subprocess.run(["bash", str(self.script), *args], env=self.env,
                              capture_output=True, text=True, timeout=5)

    def calls(self):
        path = self.root / "calls"
        return path.read_text() if path.exists() else ""

    def assert_no_handoff(self):
        self.assertNotIn("attach-device", self.calls())
        self.assertNotIn("detach-device", self.calls())
        self.assertFalse((self.root / "device.xml").exists())

    def test_default_inspect_only_inventories(self):
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Inspection only", result.stdout)
        self.assert_no_handoff()

    def test_attach_requires_explicit_interruption_consent(self):
        for args in [("attach",), ("attach", "yes"), ("attach", "--allow-host-bluetooth-interruption", "extra")]:
            self.assertNotEqual(self.run_helper(*args).returncode, 0)
        self.assert_no_handoff()

    def test_missing_or_ambiguous_radio_is_rejected(self):
        for count in ["0", "2"]:
            self.env["TEST_RADIOS"] = count
            self.assertNotEqual(self.run_helper("attach", "--allow-host-bluetooth-interruption").returncode, 0)
        self.assert_no_handoff()

    def test_session_without_usb_node_permission_is_rejected(self):
        self.env["TEST_URI"] = "qemu:///session"
        result = self.run_helper("attach", "--allow-host-bluetooth-interruption")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("read/write access", result.stderr)
        self.assert_no_handoff()

    def test_invalid_usb_identifier_cannot_inject_xml(self):
        (self.root / ".dev/vm.env").write_text(
            'export VM_LIBVIRT_DOMAIN=test-vm\n'
            'export VM_BLUETOOTH_VENDOR="abcd/><bad/>"\n'
            'export VM_BLUETOOTH_PRODUCT=1234\n'
        )
        self.assertNotEqual(self.run_helper("attach", "--allow-host-bluetooth-interruption").returncode, 0)
        self.assert_no_handoff()

    def test_approved_attach_and_detach_are_live_only_and_use_the_exact_radio(self):
        for action in ["attach", "detach"]:
            args = (action, "--allow-host-bluetooth-interruption") if action == "attach" else (action,)
            result = self.run_helper(*args)
            self.assertEqual(result.returncode, 0, result.stderr)
            call = self.calls().splitlines()[-1]
            self.assertIn(f"{action}-device --domain test-vm", call)
            self.assertIn("--live", call)
            self.assertNotIn("--config", call)
            xml_path = self.root / "device.xml"
            xml = ET.fromstring(xml_path.read_text())
            self.assertEqual(xml.attrib["type"], "usb")
            self.assertEqual(xml.find("source/vendor").attrib["id"], "0xabcd")
            self.assertEqual(xml.find("source/product").attrib["id"], "0x1234")
            original = Path(call.split("--file ")[1].split()[0])
            self.assertFalse(original.exists(), "Temporary XML must be cleaned up")


if __name__ == "__main__":
    unittest.main()
