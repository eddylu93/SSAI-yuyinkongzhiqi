using System;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Threading;

// Some Qt builds only expose a rendering surface. Identify the Simplified
// Chinese Send label in the footer and require the edit cursor above it.
public static class WeChatComposer {
    // 35 x 17 binary glyph sample at 125% Windows display scaling.
    static readonly long[] SendLabel = {
        0x1cc0060c, 0x399f1861c, 0x31831c318, 0x31800e330, 0x7fffe7ffe,
        0x300000c0, 0x300000c0, 0x7ffbc0c0, 0x7830dfff, 0xf870c0c0,
        0xc860c0e0, 0x1cce0c1f0, 0x187c0c39c, 0x30780c70e, 0x70ec0dc07,
        0x67871e000, 0x1e01f3fff
    };
    [StructLayout(LayoutKind.Sequential)]
    struct CursorInfo { public int size, flags; public IntPtr cursor; public Point point; }
    [DllImport("user32")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32")] static extern bool GetClientRect(IntPtr w, out Rectangle r);
    [DllImport("user32")] static extern bool ClientToScreen(IntPtr w, ref Point p);
    [DllImport("user32")] static extern bool ScreenToClient(IntPtr w, ref Point p);
    [DllImport("user32")] static extern uint GetDpiForWindow(IntPtr w);
    [DllImport("user32")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32")] static extern bool GetCursorPos(out Point p);
    [DllImport("user32")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32")] static extern bool GetCursorInfo(ref CursorInfo info);
    [DllImport("user32")] static extern IntPtr LoadCursor(IntPtr module, IntPtr name);
    [DllImport("user32")] static extern IntPtr WindowFromPoint(Point p);
    [DllImport("user32")] static extern IntPtr GetAncestor(IntPtr w, uint kind);
    [DllImport("user32")] static extern IntPtr SendMessageTimeout(IntPtr w, uint message,
        IntPtr wp, IntPtr lp, uint flags, uint timeout, out IntPtr result);

    static int Pixels(double n, double scale) { return (int)Math.Round(n * scale); }

    static bool HasLabel(int[,] luminance, int x, int y, double scale) {
        int background = luminance[x - Pixels(3, scale), y];
        int ink = luminance[x + Pixels(8, scale), y + Pixels(4, scale)];
        if (Math.Abs(ink - background) < 18) return false;
        int errors = 0;
        for (int row = 0; row < SendLabel.Length; row++) {
            int sy = y + Pixels(row, scale);
            for (int col = 0; col < 35; col++) {
                bool sample = Math.Abs(luminance[x + Pixels(col, scale), sy] - background)
                    > Math.Abs(ink - background) * 0.4;
                bool expected = (SendLabel[row] & (1L << (34 - col))) != 0;
                if (sample != expected && ++errors > 70) return false;
            }
        }
        return true;
    }

    static bool FindLabel(Point bottomRight, double dpiScale, out Point label) {
        label = Point.Empty;
        int width = Pixels(150, dpiScale), height = Pixels(83, dpiScale);
        int left = bottomRight.X - width, top = bottomRight.Y - height;
        int[,] luminance = new int[width,height];
        using (var image = new Bitmap(width, height))
        using (var graphics = Graphics.FromImage(image)) {
            graphics.CopyFromScreen(left, top, 0, 0, image.Size);
            for (int y = 0; y < height; y++)
                for (int x = 0; x < width; x++) {
                    Color c = image.GetPixel(x,y);
                    luminance[x,y] = (c.R + 2 * c.G + c.B) / 4;
                }
        }
        foreach (double factor in new double[] {1.0, 0.9, 1.1}) {
            double scale = dpiScale * factor;
            for (int y = Pixels(5, dpiScale); y + Pixels(17,scale) < height; y++)
                for (int x = Pixels(5, dpiScale); x + Pixels(35,scale) < width; x++) {
                    if (HasLabel(luminance,x,y,scale)) {
                        label = new Point(left+x, top+y);
                        return true;
                    }
                }
        }
        return false;
    }

    public static bool Focus(IntPtr window) {
        IntPtr oldContext = SetThreadDpiAwarenessContext(new IntPtr(-4));
        Point original = Point.Empty, edit = Point.Empty;
        bool moved = false;
        try {
            Rectangle client;
            Point origin = Point.Empty, label;
            double scale = GetDpiForWindow(window) / 120.0;
            if (scale <= 0 || GetForegroundWindow() != window ||
                !GetClientRect(window,out client) || !ClientToScreen(window,ref origin) ||
                client.Width < Pixels(300,scale) || client.Height < Pixels(200,scale)) return false;
            if (!FindLabel(new Point(origin.X+client.Width,origin.Y+client.Height),scale,out label)
                || GetAncestor(WindowFromPoint(label),2) != window) return false;
            edit = new Point(label.X-Pixels(66,scale), label.Y-Pixels(40,scale));
            IntPtr child = WindowFromPoint(edit);
            if (GetAncestor(child,2) != window || !GetCursorPos(out original)
                || GetForegroundWindow() != window || !SetCursorPos(edit.X,edit.Y)) return false;
            moved = true;
            Thread.Sleep(100);
            var info = new CursorInfo();
            info.size = Marshal.SizeOf(info);
            if (!GetCursorInfo(ref info) || info.point != edit ||
                info.cursor != LoadCursor(IntPtr.Zero,new IntPtr(32513)) ||
                GetForegroundWindow() != window || WindowFromPoint(edit) != child) return false;
            Point local = edit;
            if (!ScreenToClient(child,ref local)) return false;
            IntPtr xy = new IntPtr((local.Y << 16) | (local.X & 0xffff)), result;
            // Address the validated control directly, even if the user switches apps.
            bool down = SendMessageTimeout(child,0x201,new IntPtr(1),xy,2,350,out result) != IntPtr.Zero;
            bool up = SendMessageTimeout(child,0x202,IntPtr.Zero,xy,2,350,out result) != IntPtr.Zero;
            return down && up && GetForegroundWindow() == window;
        } finally {
            Point now;
            if (moved && GetCursorPos(out now) && now == edit) SetCursorPos(original.X,original.Y);
            if (oldContext != IntPtr.Zero) SetThreadDpiAwarenessContext(oldContext);
        }
    }

    // Also built as a small console helper for the latency-sensitive voice key.
    public static int Main(string[] args) {
        long value;
        if (args.Length != 1 || !long.TryParse(args[0], out value)) return 4;
        IntPtr window = new IntPtr(value);
        if (GetForegroundWindow() != window || !Focus(window)) return 4;
        Console.WriteLine(value);
        return 0;
    }
}
