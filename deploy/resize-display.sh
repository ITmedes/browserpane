#!/bin/bash
# Resize the Xvfb virtual display by restarting it at the new resolution.
# Xvfb's RANDR extension doesn't support runtime mode changes, so the
# only reliable way to resize is to kill Xvfb and restart it.
#
# Usage: resize-display.sh <width> <height>
#
# This also restarts openbox (window manager) and Chromium.
# The BPANE_URL env var controls which URL Chromium opens (default: last page).

set -e

WIDTH="${1:?width required}"
HEIGHT="${2:?height required}"
DISPLAY_NUM="${DISPLAY:-:99}"

write_openbox_config() {
  mkdir -p /home/bpane/.config/openbox
  cat > /home/bpane/.config/openbox/rc.xml <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<openbox_config xmlns="http://openbox.org/3.4/rc">
  <theme>
    <name>Clearlooks</name>
    <!-- Keep the frame/titlebar, but remove iconify/maximize/close buttons. -->
    <titleLayout>NL</titleLayout>
    <keepBorder>yes</keepBorder>
    <animateIconify>no</animateIconify>
    <font place="ActiveWindow"><name>sans</name><size>8</size><weight>bold</weight><slant>normal</slant></font>
    <font place="InactiveWindow"><name>sans</name><size>8</size><weight>bold</weight><slant>normal</slant></font>
    <font place="MenuHeader"><name>sans</name><size>9</size><weight>normal</weight><slant>normal</slant></font>
    <font place="MenuItem"><name>sans</name><size>9</size><weight>normal</weight><slant>normal</slant></font>
    <font place="ActiveOnScreenDisplay"><name>sans</name><size>9</size><weight>bold</weight><slant>normal</slant></font>
    <font place="InactiveOnScreenDisplay"><name>sans</name><size>9</size><weight>bold</weight><slant>normal</slant></font>
  </theme>
  <keyboard>
    <!-- Intentionally omit all keybinds so Openbox cannot close, iconify,
         move, resize, or open the client/root window menus. -->
    <chainQuitKey>C-g</chainQuitKey>
    <rebindOnMappingNotify>yes</rebindOnMappingNotify>
  </keyboard>
  <mouse>
    <!-- Keep core timing config, but omit all contexts to disable Alt-drag
         move/resize and desktop/client menu bindings. -->
    <dragThreshold>8</dragThreshold>
    <doubleClickTime>200</doubleClickTime>
    <screenEdgeWarpTime>0</screenEdgeWarpTime>
  </mouse>
  <applications>
    <!-- Match common Chromium browser windows, hide the WM frame, and pin only
         top-level normal windows maximized. Native dialogs share Chromium's
         WM_CLASS, but must keep their requested dialog size. -->
    <application class="Chromium" type="normal">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application class="chromium" type="normal">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application class="chromium-browser" type="normal">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application type="dialog">
      <decor>no</decor>
      <maximized>no</maximized>
      <position force="yes">
        <x>center</x>
        <y>center</y>
      </position>
    </application>
    <application role="GtkFileChooserDialog" type="dialog">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application type="utility">
      <decor>no</decor>
      <maximized>no</maximized>
      <position force="yes">
        <x>center</x>
        <y>center</y>
      </position>
    </application>
  </applications>
</openbox_config>
EOF
}

# Kill existing Xvfb, openbox, chromium
pkill -f "Xvfb ${DISPLAY_NUM}" 2>/dev/null || true
pkill -f openbox 2>/dev/null || true
# Don't kill chromium explicitly; it dies when X dies.

sleep 0.3

# Clean lock files
DNUM="${DISPLAY_NUM#:}"
rm -f "/tmp/.X${DNUM}-lock" "/tmp/.X11-unix/X${DNUM}"

# Start Xvfb at new resolution
Xvfb "${DISPLAY_NUM}" -screen 0 "${WIDTH}x${HEIGHT}x24" +extension RANDR &
sleep 0.5

# Restart window manager
write_openbox_config
DISPLAY="${DISPLAY_NUM}" openbox &
sleep 0.3

# Chromium on Linux reads caret blink behavior from GTK settings.
# Re-apply this before restarting the browser in case resize is used standalone.
for gtk_dir in /home/bpane/.config/gtk-3.0 /home/bpane/.config/gtk-4.0; do
  mkdir -p "$gtk_dir"
  cat > "${gtk_dir}/settings.ini" <<'EOF'
[Settings]
gtk-cursor-blink=false
gtk-cursor-blink-time=0
EOF
done

write_chromium_preferences() {
  local profile_dir="$1"
  PROFILE_DIR="$profile_dir" BPANE_DOWNLOAD_DIR="${BPANE_DOWNLOAD_DIR:-/home/bpane/bpane-downloads}" python3 - <<'PY'
import json
import os
from pathlib import Path

profile_dir = Path(os.environ["PROFILE_DIR"])
download_dir = os.environ["BPANE_DOWNLOAD_DIR"]
preferences_path = profile_dir / "Default" / "Preferences"
preferences_path.parent.mkdir(parents=True, exist_ok=True)

data = {}
if preferences_path.exists():
    try:
        data = json.loads(preferences_path.read_text())
    except Exception:
        data = {}

browser = data.setdefault("browser", {})
browser["custom_chrome_frame"] = False
download = data.setdefault("download", {})
download["default_directory"] = download_dir
download["prompt_for_download"] = False
download["directory_upgrade"] = True
profile = data.setdefault("profile", {})
default_content = profile.setdefault("default_content_setting_values", {})
default_content["media_stream_camera"] = 1
default_content["media_stream_mic"] = 1

preferences_path.write_text(json.dumps(data, separators=(",", ":")))
PY
}

# Restart Chromium (it lost its X connection)
PROFILE_DIR=/home/bpane/.bpane-chromium
mkdir -p "$PROFILE_DIR"
BPANE_UPLOAD_DIR=${BPANE_UPLOAD_DIR:-/home/bpane/bpane-uploads}
BPANE_DOWNLOAD_DIR=${BPANE_DOWNLOAD_DIR:-/home/bpane/bpane-downloads}
mkdir -p "$BPANE_UPLOAD_DIR" "$BPANE_DOWNLOAD_DIR"
write_chromium_preferences "$PROFILE_DIR"

CHROMIUM_FLAGS=(
  --no-first-run
  --no-default-browser-check
  "--user-data-dir=${PROFILE_DIR}"
  "--force-device-scale-factor=${BPANE_DEVICE_SCALE:-2}"
  "--window-size=${WIDTH},${HEIGHT}"
)

CHROMIUM_EXTENSION_DIRS=()
if [ -n "${BPANE_EXTENSION_DIRS:-}" ]; then
  IFS=',' read -r -a REQUESTED_EXTENSION_DIRS <<< "${BPANE_EXTENSION_DIRS}"
  for extension_dir in "${REQUESTED_EXTENSION_DIRS[@]}"; do
    if [ -d "${extension_dir}" ]; then
      CHROMIUM_EXTENSION_DIRS+=("${extension_dir}")
    fi
  done
fi
BPANE_EXTENSION_DIR="${BPANE_EXTENSION_DIR:-/home/bpane/bpane-ext}"
if [ -d "${BPANE_EXTENSION_DIR}" ]; then
  CHROMIUM_EXTENSION_DIRS+=("${BPANE_EXTENSION_DIR}")
fi
if [ "${#CHROMIUM_EXTENSION_DIRS[@]}" -gt 0 ]; then
  CHROMIUM_FLAGS+=("--load-extension=$(IFS=,; echo "${CHROMIUM_EXTENSION_DIRS[*]}")")
fi

chromium_log_filter() {
  grep -v \
    -e 'dbus/bus.cc' \
    -e 'dbus/object_proxy.cc' \
    -e 'registration_request.cc' \
    -e 'PHONE_REGISTRATION_ERROR' \
    -e 'QUOTA_EXCEEDED' \
    >&2
}

launch_chromium() {
  local mode="${BPANE_CHROMIUM_SANDBOX_MODE:-auto}"
  local url="${BPANE_URL:-https://example.org}"

  run_chromium_once() {
    local sandbox_mode="$1"
    shift
    case "$sandbox_mode" in
      on|strict)
        DISPLAY="${DISPLAY_NUM}" chromium "$@" 2>&1 | chromium_log_filter
        ;;
      off|disable|none)
        DISPLAY="${DISPLAY_NUM}" chromium "$@" --no-sandbox 2>&1 | chromium_log_filter
        ;;
      auto|*)
        DISPLAY="${DISPLAY_NUM}" chromium "$@" 2>&1 | chromium_log_filter &
        local pid=$!
        sleep 2
        if kill -0 "$pid" 2>/dev/null; then
          wait "$pid"
          return $?
        fi
        echo "Chromium sandbox start failed; retrying with --no-sandbox" >&2
        DISPLAY="${DISPLAY_NUM}" chromium "$@" --no-sandbox 2>&1 | chromium_log_filter
        ;;
    esac
  }

  (
    while true; do
      if run_chromium_once "$mode" "${CHROMIUM_FLAGS[@]}" "$url"; then
        exit_code=0
      else
        exit_code=$?
      fi
      echo "Chromium exited with status ${exit_code}; restarting" >&2
      sleep 1
      write_chromium_preferences "$PROFILE_DIR"
    done
  ) &
}

launch_chromium

# Wait for Chromium to render
sleep 2

echo "OK ${WIDTH}x${HEIGHT}"
