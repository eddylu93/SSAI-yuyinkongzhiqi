using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

// Windows Restart Manager asks the old Tauri process to run its exit handlers.
// Register process identities (PID + creation time), never all users of a DLL.
internal static class RunningController
{
    [StructLayout(LayoutKind.Sequential)]
    private struct AppIdentity
    {
        internal uint Pid;
        internal System.Runtime.InteropServices.ComTypes.FILETIME Started;
    }

    [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)]
    private static extern uint RmStartSession(out uint session, uint flags, StringBuilder key);
    [DllImport("rstrtmgr.dll")]
    private static extern uint RmRegisterResources(uint session, uint files, IntPtr fileNames,
        uint apps, [In] AppIdentity[] processes, uint services, IntPtr serviceNames);
    [DllImport("rstrtmgr.dll")]
    private static extern uint RmShutdown(uint session, uint flags, IntPtr progress);
    [DllImport("rstrtmgr.dll")]
    private static extern uint RmEndSession(uint session);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetLongPathName(string path, StringBuilder buffer, uint size);

    private static string FullName(string path)
    {
        var buffer = new StringBuilder(32768);
        uint length = GetLongPathName(Path.GetFullPath(path), buffer, (uint)buffer.Capacity);
        if (length == 0 || length >= buffer.Capacity) throw new IOException("Cannot resolve executable path");
        return buffer.ToString();
    }

    private static List<AppIdentity> FindRunning(string executable)
    {
        var found = new List<AppIdentity>();
        int session = Process.GetCurrentProcess().SessionId;
        foreach (var process in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(executable)))
        using (process)
        {
            try
            {
                if (process.SessionId != session ||
                    !String.Equals(FullName(process.MainModule.FileName), executable, StringComparison.OrdinalIgnoreCase))
                    continue;
                long time = process.StartTime.ToFileTimeUtc();
                found.Add(new AppIdentity {
                    Pid = (uint)process.Id,
                    Started = new System.Runtime.InteropServices.ComTypes.FILETIME {
                        dwLowDateTime = unchecked((int)time), dwHighDateTime = (int)(time >> 32)
                    }
                });
            }
            catch (InvalidOperationException) { if (!process.HasExited) throw; }
            catch (Win32Exception) { if (!process.HasExited) throw; }
        }
        return found;
    }

    // 0 = ready, 1 = still running, 2 = could not verify or close.
    private static int Main(string[] args)
    {
        try
        {
            if (args.Length != 2 || (args[0] != "check" && args[0] != "close")) return 2;
            string executable = FullName(args[1]);
            var running = FindRunning(executable);
            if (running.Count == 0) return 0;
            if (args[0] == "check") return 1;
            uint session;
            if (RmStartSession(out session, 0, new StringBuilder(33)) != 0) return 2;
            try
            {
                if (RmRegisterResources(session, 0, IntPtr.Zero, (uint)running.Count,
                        running.ToArray(), 0, IntPtr.Zero) != 0) return 2;
                RmShutdown(session, 0, IntPtr.Zero);
                return FindRunning(executable).Count == 0 ? 0 : 1;
            }
            finally { RmEndSession(session); }
        }
        catch (Exception) { return 2; }
    }
}
