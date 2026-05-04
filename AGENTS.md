<claude-mem-context>
# Memory Context

# [pot-desktop-linux] recent context, 2026-05-01 4:31pm PDT

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 40 obs (14,032t read) | 0t work

### May 1, 2026
83 3:56p ✅ All 59 tests pass after portal trigger format revision
84 3:58p 🔵 Complete screenshot capture architecture documented
85 3:59p 🔵 ashpd portal requires valid AppID matching desktop file for authentication
86 " 🔵 ashpd default features include tokio and can enable gtk4 for WindowIdentifier
87 4:00p 🔵 Non-sandboxed apps must register host app ID with portal for permissions
88 " 🔵 ashpd dependency lacks gtk4 feature — no WindowIdentifier::from_native() available
89 " 🟣 Added native Wayland screenshot tool fallbacks and portal host app registration
90 4:01p ✅ All 59 tests pass with native Wayland tools and portal registration
91 " 🔵 evdev crate explored as potential alternative for low-level key listening on Wayland
92 4:02p 🔵 evdev provides synchronized event reading with automatic state recovery
93 " 🔵 evdev KeyCode constants map to Linux input-event-codes.h values
94 " ✅ Added evdev as optional dependency for direct Linux input device hotkey support
95 4:03p ✅ Confirmed evdev dependency addition to Cargo.toml
96 " 🟣 Implemented evdev-based hotkey fallback for Wayland
97 4:04p ✅ evdev hotkey backend compiles after lifetime fix; code formatted
98 " ✅ Session summary: pot-desktop-linux bug fixes complete across 28 files
99 " ✅ Final diff cleaned to 8 core files with +870/-98 lines
100 4:11p 🔴 Wayland hotkey deduplication via time-windowed duplicate suppression
101 4:12p 🔴 Wayland hotkey backends run in parallel with shared deduplicating dispatcher
102 " 🔴 Evdev backend fully wired through HotkeyDispatcher for dedup
103 " 🔴 Screenshot region tool fallback list restricted by session type
104 " 🔴 Screenshot fallback error message distinguishes no-tools vs all-failed
105 " ✅ All hotkey and screenshot bugfix code changes complete, compilation started
106 4:13p 🔴 Complete fix for OCR black screen and hotkey failures on Wayland/X11 — full diff review
107 " 🟣 Flameshot added to screenshot region tool chain
108 " 🟣 Flameshot capture implementation uses gui --raw mode
109 4:14p ✅ All bugfix work complete — 69 tests pass, plan fully completed
110 4:15p 🔵 pot-desktop-linux build system uses Docker multi-stage with feature flags
111 4:16p 🔵 Debug binary confirms all new code paths are compiled in
112 " 🔵 Dual app ID convention: config uses com.pot-app.desktop, portal/D-Bus uses com.pot-app.pot-gtk
113 4:17p 🔵 Dev environment is Ubuntu GNOME Wayland with 18 input devices — ideal test bed for both bugs
114 " 🔵 GNOME Shell Screenshot D-Bus interface available with InteractiveScreenshot method
115 4:18p 🟣 GnomeShell D-Bus screenshot tool added as highest-priority region capture method
116 4:19p 🟣 capture_via_gnome_shell uses zbus D-Bus SelectArea + ScreenshotArea
117 " 🟣 send_local_action enables triggering app actions via localhost HTTP
118 " 🟣 CLI --pot-action flag for external hotkey triggering via local HTTP
119 4:20p 🟣 GNOME custom keybinding backend registers shortcuts via gsettings
120 " ✅ All new features compile cleanly with ocr,hotkey,tray features
121 " ✅ Full test suite passes with all new features — 69 tests green
122 4:21p ✅ Docker release build initiated with all features — deps stage rebuilding after new dependencies
</claude-mem-context>