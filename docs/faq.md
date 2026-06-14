# WinSweep — Frequently Asked Questions

## General

**Q: Is WinSweep safe to use?**  
A: Yes. WinSweep has multiple layers of protection:
- A hardcoded `NEVER_DELETE` list blocks critical system paths.
- Junctions and symlinks are never followed.
- Confirmation dialogs are shown before any deletion (configurable).
- By default, files go to the Recycle Bin, not permanent deletion.
- An audit log records every operation.

**Q: Will WinSweep delete files I need?**  
A: The scanner reports findings before anything is deleted. Review the results,
select what you want removed, and only then click Delete. You can also enable
"Move to Recycle Bin" (the default) so every deletion is recoverable.

**Q: Does WinSweep send data anywhere?**  
A: No. WinSweep is entirely offline. It makes no network connections except when
you explicitly use Docker or WSL features (which talk to local daemons only).

**Q: What Windows versions are supported?**  
A: Windows 10 (version 1903+) and Windows 11. Some features (WSL compact,
Windows Update cleanup) require Windows 10 version 2004 or later.

---

## Installation

**Q: Do I need administrator rights to install WinSweep?**  
A: The NSIS installer writes to `%ProgramFiles%` and requires elevation.
The portable ZIP version requires no installation and can run from anywhere.

**Q: Can I run WinSweep from a USB drive?**  
A: Yes. Use the portable ZIP. Configuration and logs will be written to
`%AppData%\WinSweep` on the host machine.

**Q: How do I uninstall WinSweep?**  
A: Use "Add or Remove Programs" if installed via the setup executable, or simply
delete the portable folder. To remove all data:
```
Remove-Item -Recurse "$env:APPDATA\WinSweep"
Remove-Item -Recurse "$env:LOCALAPPDATA\WinSweep"
```

---

## Scanning

**Q: Why does the scan take a long time on a large drive?**  
A: WinSweep uses a parallel walker scaled to the number of CPU cores. On HDDs,
I/O is the bottleneck. On SSDs, a full C:\ scan typically completes in under
30 seconds.

**Q: Can I scan network drives?**  
A: Yes, but results may be slower. WinSweep does not filter by drive type.

**Q: The scan found files I don't want to delete. Can I exclude them?**  
A: Uncheck the checkboxes next to those files in the results table, or add the
parent directory to the exclude list in Settings → Scan.

**Q: What does "min file size" do?**  
A: Files below the threshold (default 1 KB) are excluded from results. Increase
this to focus on large files only.

---

## Cleanup

**Q: What is the difference between "Delete" and "Move to Recycle Bin"?**  
A: "Move to Recycle Bin" uses the native Windows shell API (`SHFileOperationW`)
so files appear in the Recycle Bin and can be restored. "Delete" permanently
removes files immediately.

**Q: Can I undo a cleanup?**  
A: If "Move to Recycle Bin" was enabled, open the Recycle Bin and restore files
from there. If permanent deletion was used, the files cannot be recovered through
WinSweep (consider a file-recovery tool if you deleted something important).

**Q: I accidentally deleted something important. What can I do?**  
A: Check the Recycle Bin first. If the file was permanently deleted, use a
file-recovery tool such as Recuva or Windows File Recovery.

**Q: Will cleaning browser caches log me out of websites?**  
A: Cleaning browser caches removes temporary data (images, scripts, downloaded
pages) but does **not** delete cookies or session data. You should stay logged in.

---

## Package Manager Caches

**Q: Is it safe to clean npm/pip/cargo caches?**  
A: Yes. Package managers automatically rebuild their caches on the next `install`
or `build` command. The only cost is re-downloading packages — some tools
(Cargo, pip) will also re-compile dependencies.

**Q: Why does cleaning the Cargo registry take so long to re-populate?**  
A: Cargo's registry index and compiled dependencies can be large. Consider
keeping the registry index (`~/.cargo/registry/index`) and only deleting
`~/.cargo/registry/cache` and `~/.cargo/git/checkouts`.

**Q: WinSweep shows 0 bytes for a package manager I use. Is something wrong?**  
A: The cache path may be in a non-standard location. Check the tool's config
(e.g. `npm config get cache`, `cargo metadata`) and add the path manually in
the package managers view.

---

## Browser Caches

**Q: Which browsers does WinSweep support?**  
A: Google Chrome, Microsoft Edge (Chromium), and Mozilla Firefox. Brave, Opera,
and other Chromium-based browsers use the same cache structure but are not yet
detected automatically (their User Data directories differ).

**Q: The browser cache size is wrong / outdated.**  
A: Click **🔄 Refresh** in the Package Managers view to re-measure. Sizes are
not auto-updated until you refresh.

---

## WSL

**Q: What does "Compact Disk" do?**  
A: Linux disk images (.vhdx) grow but never automatically shrink. Compacting
runs `wsl --compact` (or `wsl --manage <distro> --set-sparse true`) which
trims unused space from the virtual disk file, freeing space on the host.

**Q: Compacting failed with "The process cannot access the file".**  
A: The distribution must be stopped before compacting. WinSweep stops it
automatically, but if another process (e.g. a running terminal) holds it open,
the operation may fail. Close all WSL terminals and try again.

**Q: Will unregistering a distribution delete my files?**  
A: Yes. Unregistering is permanent and cannot be undone. Back up important files
first (`wsl --export <distro> backup.tar`).

---

## Scheduled Tasks

**Q: How does the scheduled task work?**  
A: When you click "Register Startup Task" in Settings → Cleanup, WinSweep uses
`schtasks.exe` to create an **ONLOGON** task that runs `winsweep-gui.exe`
automatically at logon with a 1-minute delay.

**Q: The scheduled task runs but I don't see any cleanup happening.**  
A: The GUI opens normally when the task fires. Auto-cleanup only runs if
"Enable automatic cleanup" is checked and the configured interval has elapsed.
Check the Recent Activity log on the Dashboard.

**Q: Can I run WinSweep as a background service instead?**  
A: Not natively. Consider using the CLI in ndjson mode and scheduling it with
Task Scheduler as a `DAILY` hidden task instead.

---

## Performance & Resources

**Q: How much RAM does WinSweep use?**  
A: The base GUI footprint is approximately 50–80 MB. Large scans temporarily
increase this as results are held in memory.

**Q: Does WinSweep slow down my computer while scanning?**  
A: The scanner is CPU and I/O intensive but yields between files. A full C:\
scan at full parallelism uses one CPU core equivalently. You can reduce
`max_concurrent_operations` in Settings → Advanced to limit parallelism.

**Q: WinSweep's window feels laggy.**  
A: egui targets 60 fps. On HiDPI monitors or slow GPUs, you may want to reduce
the window size or disable the plot charts in Settings → Advanced.

---

## Errors & Troubleshooting

**Q: "Access is denied" when deleting.**  
A: Some files are locked by running processes. WinSweep uses the Windows Restart
Manager API to detect and optionally close those processes. If the option is
disabled, try closing the application that owns the file and retrying.

**Q: The elevated coordinator failed to start.**  
A: This usually means UAC is blocking the helper process. Check that UAC is
not set to "Never notify" in User Account Control settings (this actually blocks
elevation entirely in some configurations).

**Q: "schtasks.exe not found" when registering a scheduled task.**  
A: This is unexpected on any Windows installation. Verify that
`C:\Windows\System32\schtasks.exe` exists and that `%SystemRoot%\System32` is
in your `PATH`.
