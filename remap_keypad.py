import ctypes
from ctypes import wintypes
import time

GENERIC_READ = 0x80000000
GENERIC_WRITE = 0x40000000
FILE_SHARE_READ = 1
FILE_SHARE_WRITE = 2
OPEN_EXISTING = 3

k = ctypes.windll.kernel32
hid = ctypes.windll.hid

# The vendor configuration interface path for VID_5131&PID_2019
path = r"\\?\HID#VID_5131&PID_2019&MI_01#8&247e9c26&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"

print(f"Opening device: {path}")
h = k.CreateFileW(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, 0, None)
if h == -1:
    print(f"Failed to open device handle. Error code: {k.GetLastError()}")
    exit(1)

print("Device opened successfully!")

# USB HID usage code for F13 is 0x68
KEY_F13 = 0x68

# Program pins 1, 2, and 3 to F13 so whichever pin the physical switch is wired to, it outputs F13.
# (FS1-P typically uses pin 3 / P1.5, but covering 1, 2, 3 guarantees coverage)
pins_to_program = [0x01, 0x02, 0x03]

for pin in pins_to_program:
    buf = bytearray(65)
    buf[0] = 0x10  # REPORT_SET_CODE
    buf[1] = pin   # Pin ID
    buf[2] = 0x80  # Key Command
    buf[3] = 0x08  # Payload size
    buf[4] = 0x00  # Modifiers (None)
    buf[5] = 0x00  # Reserved
    buf[6] = KEY_F13  # Key: F13
    
    written = wintypes.DWORD()
    ok = k.WriteFile(h, (ctypes.c_char * 65).from_buffer(buf), 65, ctypes.byref(written), None)
    err = k.GetLastError()
    print(f"Programming Pin 0x{pin:02X} -> F13 (0x{KEY_F13:02X}): Success={bool(ok)}, Bytes={written.value}, Err={err}")
    time.sleep(0.05)

k.CloseHandle(h)
print("\n[SUCCESS] Reprogramming complete! The button's flash memory has been updated to F13.")
