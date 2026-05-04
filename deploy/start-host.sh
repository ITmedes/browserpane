#!/bin/bash
set -e

normalize_path() {
  python3 - "$1" <<'PY'
import sys
from pathlib import Path

print(Path(sys.argv[1]).resolve(strict=False))
PY
}

path_is_within() {
  local child="$1"
  local root="$2"
  case "$child" in
    "$root"|"$root"/*) return 0 ;;
    *) return 1 ;;
  esac
}

validate_session_data_path() {
  local name="$1"
  local value="$2"
  local root="$3"
  local normalized
  normalized="$(normalize_path "$value")"
  if ! path_is_within "$normalized" "$root"; then
    echo "invalid ${name}: ${value} escapes BPANE_SESSION_DATA_DIR=${root}" >&2
    exit 64
  fi
}

# Create a dedicated Chromium profile and transfer roots for the current
# BrowserPane session. Docker-backed runtimes provide BPANE_SESSION_DATA_DIR and
# must keep browser data below that root; static_single remains a legacy path.
BPANE_SESSION_ID="${BPANE_SESSION_ID:-static-default}"
BPANE_PROFILE_ROOT="${BPANE_PROFILE_ROOT:-/home/bpane/.bpane-chromium-sessions}"
SESSION_STATE_DIR="${BPANE_PROFILE_ROOT%/}/${BPANE_SESSION_ID}"
PROFILE_DIR="${BPANE_PROFILE_DIR:-${SESSION_STATE_DIR}/chromium}"
BPANE_UPLOAD_DIR="${BPANE_UPLOAD_DIR:-${SESSION_STATE_DIR}/uploads}"
BPANE_DOWNLOAD_DIR="${BPANE_DOWNLOAD_DIR:-${SESSION_STATE_DIR}/downloads}"

if [ -n "${BPANE_SESSION_DATA_DIR:-}" ]; then
  BPANE_SESSION_DATA_DIR="$(normalize_path "$BPANE_SESSION_DATA_DIR")"
  if [ "$BPANE_SESSION_DATA_DIR" = "/" ]; then
    echo "invalid BPANE_SESSION_DATA_DIR: refusing to use / as the session data root" >&2
    exit 64
  fi
  validate_session_data_path "BPANE_PROFILE_DIR" "$PROFILE_DIR" "$BPANE_SESSION_DATA_DIR"
  validate_session_data_path "BPANE_UPLOAD_DIR" "$BPANE_UPLOAD_DIR" "$BPANE_SESSION_DATA_DIR"
  validate_session_data_path "BPANE_DOWNLOAD_DIR" "$BPANE_DOWNLOAD_DIR" "$BPANE_SESSION_DATA_DIR"
  export BPANE_SESSION_DATA_DIR
fi
export BPANE_SESSION_ID BPANE_PROFILE_ROOT PROFILE_DIR BPANE_UPLOAD_DIR BPANE_DOWNLOAD_DIR

if [ "${BPANE_VALIDATE_RUNTIME_PATHS_ONLY:-0}" = "1" ]; then
  exit 0
fi

# Clean stale lock files
rm -f /tmp/.X99-lock /tmp/.X11-unix/X99

# Start Xorg with dummy video driver (supports proper xrandr mode switching).
# -nocursor hides the hardware cursor; we draw the remote cursor in the client.
# Redirect Xorg stderr to suppress harmless xkbcomp keysym warnings and
# the _XSERVTransmkdir notice (non-root can't create /tmp/.X11-unix).
Xorg :99 -noreset -config /etc/X11/xorg-dummy.conf -nocursor 2>/dev/null &
sleep 1

export DISPLAY=:99

# Set initial resolution via xrandr
xrandr --newmode "1280x720_60.00" 74.48 1280 1344 1472 1664 720 723 728 748 -hsync +vsync 2>/dev/null || true
xrandr --addmode DUMMY0 "1280x720_60.00" 2>/dev/null || true
xrandr --output DUMMY0 --mode "1280x720_60.00" 2>/dev/null || true
# Apply DPI scaling for larger UI (Chromium chrome + web content)
BPANE_DPI=${BPANE_DPI:-144}
xrandr --dpi "$BPANE_DPI" 2>/dev/null || true

# Generate a locked-down Openbox config for the embedded Chromium window.
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
    <!-- Match common Chromium WM_CLASS values, hide the WM frame, and pin them
         maximized. Chromium still uses the native frame path internally, but
         Openbox does not render that frame. -->
    <application class="Chromium">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application class="chromium">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <application class="chromium-browser">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
    <!-- Fallback: all normal windows stay undecorated and start maximized. -->
    <application type="normal">
      <decor>no</decor>
      <maximized>yes</maximized>
    </application>
  </applications>
</openbox_config>
EOF
}

# Start openbox window manager
write_openbox_config
openbox &
sleep 0.5
openbox --reconfigure

# GTK scale factors (integer + fractional tweak)
export GDK_SCALE=${GDK_SCALE:-3}
export GDK_DPI_SCALE=${GDK_DPI_SCALE:-0.8}

# Chromium on Linux reads caret blink behavior from GTK settings.
# Disable blinking to avoid periodic damage on otherwise static pages.
for gtk_dir in /home/bpane/.config/gtk-3.0 /home/bpane/.config/gtk-4.0; do
  mkdir -p "$gtk_dir"
  cat > "${gtk_dir}/settings.ini" <<'EOF'
[Settings]
gtk-cursor-blink=false
gtk-cursor-blink-time=0
EOF
done

mkdir -p "$PROFILE_DIR" "$BPANE_UPLOAD_DIR" "$BPANE_DOWNLOAD_DIR"
rm -f \
  "${PROFILE_DIR}/SingletonLock" \
  "${PROFILE_DIR}/SingletonSocket" \
  "${PROFILE_DIR}/SingletonCookie"

write_chromium_preferences() {
  local profile_dir="$1"
  PROFILE_DIR="$profile_dir" BPANE_DOWNLOAD_DIR="$BPANE_DOWNLOAD_DIR" python3 - <<'PY'
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
# Force Chromium onto the native WM frame so Openbox owns the titlebar buttons.
browser["custom_chrome_frame"] = False
browser["restore_on_startup"] = 1
browser["restore_on_startup_migrated"] = True
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

# Chromium runtime flags tuned to keep rendering deterministic and reduce
# background activity/noise similar to our previous Firefox profile.
# Keep background networking enabled so policy-installed extensions can fetch.
CHROMIUM_FLAGS=(
  --no-first-run
  --no-default-browser-check
  --disable-background-timer-throttling
  --disable-breakpad
  --disable-client-side-phishing-detection
  --disable-component-update
  --disable-default-apps
  --disable-domain-reliability
  --disable-features=Translate,MediaRouter,OptimizationHints,AutofillServerCommunication,SafetyCheck,CalculateNativeWinOcclusion,PushMessaging
  --disable-gpu
  --disable-gpu-compositing
  --disable-renderer-backgrounding
  --disable-smooth-scrolling
  --disable-sync
  --disable-image-animation-resync
  --autoplay-policy=user-gesture-required
  --force-prefers-reduced-motion
  --metrics-recording-only
  --password-store=basic
  --restore-last-session
  --hide-crash-restore-bubble
  --use-gl=swiftshader
  --ozone-platform=x11
  "--user-data-dir=${PROFILE_DIR}"
  "--force-device-scale-factor=${BPANE_DEVICE_SCALE:-2}"
  "--window-size=${BPANE_WINDOW_SIZE:-1280,720}"
)

# Load unpacked extensions from runtime-provided directories plus the default
# local override extension when present. Keep this additive so policy-installed
# extensions such as AdBlock remain active.
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

if [ "${BPANE_CHROMIUM_DEBUG_ENABLE:-1}" != "0" ]; then
  CHROMIUM_FLAGS+=(
    "--remote-debugging-address=${BPANE_CHROMIUM_DEBUG_ADDRESS:-0.0.0.0}"
    "--remote-debugging-port=${BPANE_CHROMIUM_DEBUG_PORT:-9222}"
  )
fi

if [ -n "${BPANE_CHROMIUM_EXTRA_FLAGS:-}" ]; then
  # shellcheck disable=SC2206
  EXTRA_CHROMIUM_FLAGS=(${BPANE_CHROMIUM_EXTRA_FLAGS})
  CHROMIUM_FLAGS+=("${EXTRA_CHROMIUM_FLAGS[@]}")
fi

write_chromium_preferences "$PROFILE_DIR"

CHROMIUM_PIPE_PID=""

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
  local url="$1"
  local mode="${BPANE_CHROMIUM_SANDBOX_MODE:-auto}"
  # When --app= is in the flags, skip the positional URL to avoid opening a second window.
  local url_args=("$url")
  for flag in "${CHROMIUM_FLAGS[@]}"; do
    if [[ "$flag" == --app=* ]]; then
      url_args=()
      break
    fi
  done
  run_chromium_once() {
    local sandbox_mode="$1"
    shift
    case "$sandbox_mode" in
      on|strict)
        chromium "$@" 2>&1 | chromium_log_filter
        ;;
      off|disable|none)
        chromium "$@" --no-sandbox 2>&1 | chromium_log_filter
        ;;
      auto|*)
        chromium "$@" 2>&1 | chromium_log_filter &
        CHROMIUM_PIPE_PID=$!
        sleep 2
        if kill -0 "$CHROMIUM_PIPE_PID" 2>/dev/null; then
          wait "$CHROMIUM_PIPE_PID"
          return $?
        fi
        echo "Chromium sandbox start failed; retrying with --no-sandbox" >&2
        chromium "$@" --no-sandbox 2>&1 | chromium_log_filter
        ;;
    esac
  }

  (
    while true; do
      if run_chromium_once "$mode" "${CHROMIUM_FLAGS[@]}" "${url_args[@]}"; then
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

# Start PipeWire audio stack (provides PulseAudio compat for FFmpeg capture)
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/runtime-bpane}"
mkdir -p "$XDG_RUNTIME_DIR"
# D-Bus session bus is required by PipeWire and WirePlumber
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS

# Suppress libcamera IPA warnings (not needed for our use case)
export LIBCAMERA_LOG_LEVELS="*:ERROR"

# Suppress PipeWire RT-scheduling and JACK detection noise (no system bus in container).
# PipeWire and WirePlumber still function correctly without RT priority.
pipewire 2>&1 | grep -v -e 'mod.rt' -e 'pw_rtkit_bus_get' -e 'rtkit_get_bus' >&2 &
sleep 0.3
pipewire-pulse 2>&1 | grep -v -e 'mod.rt' -e 'pw_rtkit_bus_get' -e 'rtkit_get_bus' >&2 &
sleep 0.3
wireplumber 2>&1 | grep -v -e 'mod.rt' -e 'pw_rtkit_bus_get' -e 'rtkit_get_bus' -e 'jackdbus' >&2 &
sleep 0.5

# Create a dedicated desktop audio sink for capture.
# Apps play to this sink; FFmpeg captures from its monitor.
# This prevents the mic virtual source from interfering with audio capture.
pactl load-module module-null-sink sink_name=bpane-desktop \
  sink_properties=device.description=BrowserPane-Desktop-Audio \
  format=s16le rate=48000 channels=2 > /dev/null
pactl set-default-sink bpane-desktop
pactl set-default-source bpane-desktop.monitor

# Launch Chromium with full chrome (tabs + URL bar).
# Sandbox mode is auto by default: fallback to --no-sandbox only if required.
launch_chromium "${BPANE_URL:-https://example.org}"
sleep 5

# Chromium ignores --remote-debugging-address on newer versions and binds CDP
# to 127.0.0.1 only. Proxy it on a separate port (default 9223) bound to all
# interfaces so the MCP bridge container can reach it via the Docker network.
if [ "${BPANE_CHROMIUM_DEBUG_ENABLE:-1}" != "0" ]; then
  CDP_PORT="${BPANE_CHROMIUM_DEBUG_PORT:-9222}"
  CDP_PROXY_PORT="${BPANE_CDP_PROXY_PORT:-9223}"
  socat TCP-LISTEN:"${CDP_PROXY_PORT}",fork,bind=0.0.0.0,reuseaddr TCP:127.0.0.1:"${CDP_PORT}" &
fi

# Start the BrowserPane host agent
BPANE_SOCKET_PATH="${BPANE_SOCKET_PATH:-/run/bpane/agent.sock}"
mkdir -p "$(dirname "$BPANE_SOCKET_PATH")"
exec /usr/local/bin/bpane-host --socket "$BPANE_SOCKET_PATH" --fps "${BPANE_FPS:-30}"
