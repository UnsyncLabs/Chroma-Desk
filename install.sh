#!/usr/bin/env bash
# chroma-desk installer for Linux and macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/UnsyncLabs/Chroma-Desk/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/UnsyncLabs/Chroma-Desk/main/install.sh | bash -s -- --yes --razer
#
# It installs OpenRGB (the Razer-capable "openrgb-razer" build when you have Razer devices on Linux,
# the official OpenRGB otherwise) and then chroma-desk. Windows: use install.ps1.
#
# Security: only HTTPS downloads from github.com/UnsyncLabs, every package is checked against the
# release's SHA256SUMS, sudo is used only for the steps that need it (the command is printed first)
# and the script refuses to run as root.
set -euo pipefail

REPO="UnsyncLabs/Chroma-Desk"
API="https://api.github.com/repos/${REPO}"
DL_PREFIX="https://github.com/${REPO}/releases/download/"

ASSUME_YES=0
DRY_RUN=0
UNINSTALL=0
RAZER=auto # auto | yes | no
CD_VERSION=""
RAZER_VERSION=""

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
  es*) LANG_ES=1 ;;
  *) LANG_ES=0 ;;
esac

# t "English text" "Texto en español"
t() { if [ "$LANG_ES" = 1 ]; then printf '%s' "$2"; else printf '%s' "$1"; fi; }
info() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die() {
  printf '\033[1;31merror:\033[0m %s\n' "$*" >&2
  exit 1
}

usage() {
  if [ "$LANG_ES" = 1 ]; then
    cat <<'EOF'
Uso: install.sh [opciones]

  --razer            instala openrgb-razer (compatible con dispositivos Razer en Linux)
  --no-razer         instala el OpenRGB oficial
  --yes, -y          no hace preguntas (usa lo detectado y las respuestas por defecto)
  --uninstall        quita chroma-desk y, si este script instaló openrgb-razer, vuelve al OpenRGB oficial
  --version vX.Y.Z   versión de chroma-desk (por defecto la última)
  --razer-version T  etiqueta de openrgb-razer (por defecto la última, p. ej. openrgb-razer-v1.1-r1)
  --dry-run          muestra lo que haría sin instalar nada
  --help, -h         esta ayuda
EOF
  else
    cat <<'EOF'
Usage: install.sh [options]

  --razer            install openrgb-razer (supports Razer devices on Linux)
  --no-razer         install the official OpenRGB
  --yes, -y          ask nothing (use what was detected and the default answers)
  --uninstall        remove chroma-desk and, if this script installed openrgb-razer, go back to official OpenRGB
  --version vX.Y.Z   chroma-desk version (default: latest)
  --razer-version T  openrgb-razer tag (default: latest, e.g. openrgb-razer-v1.1-r1)
  --dry-run          show what would be done without installing anything
  --help, -h         this help
EOF
  fi
}

while [ $# -gt 0 ]; do
  case "$1" in
    --razer) RAZER=yes ;;
    --no-razer) RAZER=no ;;
    --yes | -y) ASSUME_YES=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --version)
      [ $# -ge 2 ] || die "--version needs a value"
      CD_VERSION="$2"
      shift
      ;;
    --version=*) CD_VERSION="${1#*=}" ;;
    --razer-version)
      [ $# -ge 2 ] || die "--razer-version needs a value"
      RAZER_VERSION="$2"
      shift
      ;;
    --razer-version=*) RAZER_VERSION="${1#*=}" ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "$(t "unknown option: $1 (see --help)" "opción desconocida: $1 (mira --help)")" ;;
  esac
  shift
done

if [ "$(id -u)" -eq 0 ]; then
  die "$(t "do not run this as root; it asks for sudo only where needed." "no lo ejecutes como root; pide sudo solo donde hace falta.")"
fi

TAG_RE='^[A-Za-z0-9._-]+$'
for v in "$CD_VERSION" "$RAZER_VERSION"; do
  if [ -n "$v" ] && ! [[ $v =~ $TAG_RE ]]; then die "invalid version/tag: $v"; fi
done

# ---------------------------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------------------------

TMP=""
cleanup() { [ -z "$TMP" ] || rm -rf "$TMP"; }
trap cleanup EXIT
mk_tmp() { [ -n "$TMP" ] || TMP="$(mktemp -d)"; }

# Asks a yes/no question. $3 is the default (y/n). With --yes (or without a terminal) the default is used.
confirm() {
  local q="$1" def="$3" ans="" hint
  [ "$def" = y ] && hint="[$(t "Y/n" "S/n")]" || hint="[$(t "y/N" "s/N")]"
  if [ "$ASSUME_YES" = 1 ]; then
    if [ "$def" = y ]; then return 0; else return 1; fi
  fi
  # stdin may be the script itself (curl | bash): read the answer from the terminal.
  if { true </dev/tty; } 2>/dev/null; then
    printf '%s %s ' "$(t "$q" "$2")" "$hint" >/dev/tty
    read -r ans </dev/tty || ans=""
  else
    warn "$(t "no terminal to ask: using the default ($def) for: $q" "sin terminal para preguntar: uso la respuesta por defecto ($def) para: $2")"
  fi
  case "${ans:-$def}" in
    [yYsS]*) return 0 ;;
    *) return 1 ;;
  esac
}

# Prints a command, then runs it unless --dry-run.
run() {
  printf '  + %s\n' "$*"
  [ "$DRY_RUN" = 1 ] || "$@"
}

# Same, through sudo (never when already root: the script refuses to run as root anyway).
run_sudo() {
  printf '  + sudo %s\n' "$*"
  [ "$DRY_RUN" = 1 ] && return 0
  command -v sudo >/dev/null 2>&1 || die "$(t "sudo is required for this step; install it or run the command above as root." "hace falta sudo para este paso; instálalo o ejecuta el comando de arriba como root.")"
  # Package managers must not read their prompts from our stdin (it may be the piped script).
  sudo "$@" </dev/null
}

CURL=(curl -fsSL --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 20)

http_get() { "${CURL[@]}" -H 'Accept: application/vnd.github+json' "$1"; }

# download URL DEST: only assets of this repository's releases.
download() {
  local url="$1" dest="$2"
  [[ $url == "${DL_PREFIX}"* ]] || die "refusing to download from outside ${DL_PREFIX}: $url"
  "${CURL[@]}" -o "$dest" "$url" || die "$(t "download failed: $url" "falló la descarga: $url")"
}

have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------------------------------------
# platform detection
# ---------------------------------------------------------------------------------------------

OS="$(uname -s)"
case "$OS" in
  Linux | Darwin) ;;
  *) die "$(t "unsupported system: $OS (on Windows use install.ps1)" "sistema no soportado: $OS (en Windows usa install.ps1)")" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ARCH=x86_64 ;;
  aarch64 | arm64) ARCH=aarch64 ;;
  *) die "$(t "unsupported architecture: $(uname -m)" "arquitectura no soportada: $(uname -m)")" ;;
esac
case "$ARCH" in
  x86_64)
    DEB_ARCH=amd64
    ;;
  *)
    DEB_ARCH=arm64
    ;;
esac

FAMILY=appimage # arch | deb | rpm | appimage
PRETTY="$OS"
if [ "$OS" = Linux ] && [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  osr="$(
    . /etc/os-release
    printf '%s|%s|%s' "${ID:-}" "${ID_LIKE:-}" "${PRETTY_NAME:-Linux}"
  )"
  os_id="${osr%%|*}"
  rest="${osr#*|}"
  os_like="${rest%%|*}"
  PRETTY="${rest#*|}"
  for w in $os_id $os_like; do
    case "$w" in
      arch | manjaro | cachyos | endeavouros | garuda | archarm) FAMILY=arch && break ;;
      debian | ubuntu | linuxmint | pop | raspbian | neon | zorin | elementary) FAMILY=deb && break ;;
      fedora | rhel | centos | rocky | almalinux | nobara | opensuse* | suse | sles) FAMILY=rpm && break ;;
    esac
  done
fi

# The openrgb-razer .deb is built on Debian 13 and the .rpm on Fedora 44: other derivatives (Ubuntu,
# openSUSE...) have different library versions and get the AppImage instead. Arch is built on Arch.
RAZER_NATIVE=0
case "$FAMILY:${os_id:-}" in
  arch:*) RAZER_NATIVE=1 ;;
  deb:debian | deb:raspbian) RAZER_NATIVE=1 ;;
  rpm:fedora | rpm:nobara) RAZER_NATIVE=1 ;;
esac

RPM_TOOL=""
if [ "$FAMILY" = rpm ]; then
  if have dnf; then
    RPM_TOOL=dnf
  elif have zypper; then
    RPM_TOOL=zypper
  else FAMILY=appimage; fi
fi
# Package managers that are not installed cannot be used even if the distro says so.
case "$FAMILY" in
  arch) have pacman || FAMILY=appimage ;;
  deb) have apt-get || FAMILY=appimage ;;
esac

# ---------------------------------------------------------------------------------------------
# release lookup (GitHub API, no jq needed)
# ---------------------------------------------------------------------------------------------

# latest_tag PREFIX_REGEX: newest published, non-prerelease release whose tag matches.
latest_tag() {
  local re="$1" json
  json="$(http_get "${API}/releases?per_page=100")" || die "$(t "could not reach the GitHub API" "no se pudo contactar con la API de GitHub")"
  printf '%s\n' "$json" | awk -v re="$re" '
    /^    "tag_name":/ { tag = $2; gsub(/[",]/, "", tag) }
    /^    "prerelease":/ { pre = $2; gsub(/,/, "", pre); if (!done && tag ~ re && pre == "false") { print tag; done = 1 } tag = "" }'
}

# release_assets TAG: the browser_download_url of every asset.
release_assets() {
  local json
  json="$(http_get "${API}/releases/tags/$1")" || die "$(t "release $1 not found" "no se encontró el release $1")"
  printf '%s\n' "$json" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/^[^:]*: *"//; s/"$//; s/%2[Bb]/+/g' |
    grep -E "^${DL_PREFIX//./\\.}[A-Za-z0-9._-]+/[A-Za-z0-9._+-]+$" || true
}

# pick_asset "URLS" REGEX: the first URL whose file name matches.
pick_asset() {
  local u
  while IFS= read -r u; do
    [ -n "$u" ] || continue
    if [[ ${u##*/} =~ $2 ]]; then
      printf '%s' "$u"
      return 0
    fi
  done <<<"$1"
  return 1
}

# fetch_verified TAG URL: downloads URL into $TMP and checks it against the release's SHA256SUMS.
fetch_verified() {
  local tag="$1" url="$2" name sums
  name="${url##*/}"
  mk_tmp
  if [ "$DRY_RUN" = 1 ]; then
    printf '  + download %s (+ SHA256SUMS check)\n' "$url"
    FETCHED="$TMP/$name"
    return 0
  fi
  info "$(t "Downloading $name" "Descargando $name")"
  download "$url" "$TMP/$name"
  download "${DL_PREFIX}${tag}/SHA256SUMS" "$TMP/SHA256SUMS.$tag"
  sums="$(awk -v f="$name" '$2 == f || $2 == "*" f { print $1 "  " f; exit }' "$TMP/SHA256SUMS.$tag")"
  [ -n "$sums" ] || die "$(t "$name is not listed in SHA256SUMS: refusing to install it" "$name no aparece en SHA256SUMS: no lo instalo")"
  if have sha256sum; then
    (cd "$TMP" && printf '%s\n' "$sums" | sha256sum -c - >/dev/null) || die "$(t "checksum mismatch for $name" "la suma SHA-256 de $name no coincide")"
  else
    (cd "$TMP" && printf '%s\n' "$sums" | shasum -a 256 -c - >/dev/null) || die "$(t "checksum mismatch for $name" "la suma SHA-256 de $name no coincide")"
  fi
  info "$(t "SHA-256 verified: $name" "SHA-256 verificado: $name")"
  FETCHED="$TMP/$name"
}

FETCHED=""

# ---------------------------------------------------------------------------------------------
# state (what this script installed, for --uninstall)
# ---------------------------------------------------------------------------------------------

STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/chroma-desk"
STATE_FILE="$STATE_DIR/installer.state"
state_get() { [ -r "$STATE_FILE" ] && sed -n "s/^$1=//p" "$STATE_FILE" | head -1 || true; }
state_set() {
  [ "$DRY_RUN" = 1 ] && return 0
  mkdir -p "$STATE_DIR"
  { [ -r "$STATE_FILE" ] && grep -v "^$1=" "$STATE_FILE" || true; } >"$STATE_FILE.new"
  printf '%s=%s\n' "$1" "$2" >>"$STATE_FILE.new"
  mv "$STATE_FILE.new" "$STATE_FILE"
}
state_clear() { [ "$DRY_RUN" = 1 ] || rm -f "$STATE_FILE"; }

# ---------------------------------------------------------------------------------------------
# Razer detection
# ---------------------------------------------------------------------------------------------

detect_razer() {
  RAZER_FOUND=()
  local d vid prod
  for d in /sys/bus/usb/devices/*; do
    [ -r "$d/idVendor" ] || continue
    vid="$(cat "$d/idVendor" 2>/dev/null || true)"
    if [ "$vid" = 1532 ]; then
      prod="$(cat "$d/product" 2>/dev/null || true)"
      RAZER_FOUND+=("${prod:-Razer device} (1532:$(cat "$d/idProduct" 2>/dev/null || echo '????'))")
    fi
  done
  if [ ${#RAZER_FOUND[@]} -eq 0 ] && have lsusb; then
    while IFS= read -r line; do
      [ -n "$line" ] && RAZER_FOUND+=("$line")
    done < <(lsusb 2>/dev/null | grep -i ' ID 1532:' | sed 's/^Bus [0-9]* Device [0-9]*: //' || true)
  fi
}

# ---------------------------------------------------------------------------------------------
# package-manager actions
# ---------------------------------------------------------------------------------------------

pm_install_local() { # FILE
  case "$FAMILY" in
    arch) run_sudo pacman -U --noconfirm --ask=4 "$1" ;;
    deb)
      run_sudo apt-get update
      run_sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y "$1"
      ;;
    rpm)
      if [ "$RPM_TOOL" = dnf ]; then
        run_sudo dnf install -y "$1"
      else
        run_sudo zypper --non-interactive install --allow-unsigned-rpm "$1"
      fi
      ;;
  esac
}

pm_install_repo() { # PACKAGE
  case "$FAMILY" in
    arch) run_sudo pacman -S --needed --noconfirm "$1" ;;
    deb)
      run_sudo apt-get update
      run_sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y "$1"
      ;;
    rpm)
      if [ "$RPM_TOOL" = dnf ]; then
        run_sudo dnf install -y "$1"
      else
        run_sudo zypper --non-interactive install "$1"
      fi
      ;;
  esac
}

pm_remove() { # PACKAGE
  case "$FAMILY" in
    arch) run_sudo pacman -Rns --noconfirm "$1" ;;
    deb) run_sudo env DEBIAN_FRONTEND=noninteractive apt-get remove -y "$1" ;;
    rpm)
      if [ "$RPM_TOOL" = dnf ]; then
        run_sudo dnf remove -y "$1"
      else
        run_sudo zypper --non-interactive remove "$1"
      fi
      ;;
  esac
}

pkg_installed() { # PACKAGE
  case "$FAMILY" in
    arch) pacman -Qq "$1" >/dev/null 2>&1 ;;
    deb) dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q 'install ok installed' ;;
    rpm) rpm -q "$1" >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}

# asset name patterns per family
razer_pkg_re() {
  case "$FAMILY" in
    arch) [ "$ARCH" = x86_64 ] && echo '^openrgb-razer-.*-x86_64\.pkg\.tar\.zst$' ;;
    deb) echo "^openrgb-razer_.*_${DEB_ARCH}\\.deb\$" ;;
    rpm) echo "^openrgb-razer-.*\\.${ARCH}\\.rpm\$" ;;
  esac
}
razer_appimage_re() { echo "^OpenRGB-razer.*${ARCH}\\.AppImage\$|^openrgb-razer.*${ARCH}\\.AppImage\$"; }
cd_pkg_re() {
  case "$FAMILY" in
    arch) [ "$ARCH" = x86_64 ] && echo '^chroma-desk-.*-x86_64\.pkg\.tar\.zst$' ;;
    deb) echo "^chroma-desk_.*_${DEB_ARCH}\\.deb\$" ;;
    rpm) echo "^chroma-desk-.*\\.${ARCH}\\.rpm\$" ;;
  esac
}
cd_appimage_re() { echo "^chroma-desk_.*_(${DEB_ARCH}|${ARCH})\\.AppImage\$"; }

# ---------------------------------------------------------------------------------------------
# udev rules for the AppImage route
# ---------------------------------------------------------------------------------------------

install_udev_rules() { # TAG URLS
  local url
  url="$(pick_asset "$2" '^60-openrgb\.rules$')" || {
    warn "$(t "this release has no udev rules file; devices may need extra permissions" "este release no trae reglas udev; los dispositivos pueden necesitar permisos adicionales")"
    return 0
  }
  fetch_verified "$1" "$url"
  run_sudo install -D -m 644 "$FETCHED" /etc/udev/rules.d/60-openrgb.rules
}

reload_udev() {
  have udevadm || return 0
  run_sudo udevadm control --reload-rules || warn "$(t "could not reload udev rules (not fatal)" "no se pudieron recargar las reglas udev (no es grave)")"
  run_sudo udevadm trigger || true
}

# ---------------------------------------------------------------------------------------------
# steps
# ---------------------------------------------------------------------------------------------

install_openrgb_razer() {
  local tag="${RAZER_VERSION:-}" urls url re
  if [ -z "$tag" ]; then
    tag="$(latest_tag '^openrgb-razer-v')"
    [ -n "$tag" ] || die "$(t "no openrgb-razer release found yet" "todavía no hay ningún release de openrgb-razer")"
  fi
  info "$(t "Installing openrgb-razer ($tag) in place of the official OpenRGB" "Instalando openrgb-razer ($tag) en lugar del OpenRGB oficial")"
  urls="$(release_assets "$tag")"
  [ -n "$urls" ] || die "$(t "release $tag has no downloadable files" "el release $tag no tiene archivos")"
  re="$(razer_pkg_re || true)"
  if [ "$FAMILY" != appimage ] && [ "$RAZER_NATIVE" = 1 ] && [ -n "$re" ] && url="$(pick_asset "$urls" "$re")"; then
    fetch_verified "$tag" "$url"
    pm_install_local "$FETCHED"
    state_set razer native
  else
    url="$(pick_asset "$urls" "$(razer_appimage_re)")" || die "$(t "no openrgb-razer package for $PRETTY ($ARCH)" "no hay paquete de openrgb-razer para $PRETTY ($ARCH)")"
    fetch_verified "$tag" "$url"
    run mkdir -p "$HOME/.local/bin"
    run install -m 755 "$FETCHED" "$HOME/.local/bin/openrgb"
    install_udev_rules "$tag" "$urls"
    state_set razer appimage
  fi
}

install_openrgb_official() {
  if have openrgb || { [ "$OS" = Linux ] && [ "$FAMILY" != appimage ] && pkg_installed openrgb; }; then
    info "$(t "OpenRGB is already installed: leaving it as it is" "OpenRGB ya está instalado: lo dejo como está")"
    return 0
  fi
  info "$(t "Installing the official OpenRGB" "Instalando el OpenRGB oficial")"
  if [ "$OS" = Darwin ]; then
    if have brew; then
      run brew install --cask openrgb
    else
      warn "$(t "Homebrew not found: install OpenRGB from https://openrgb.org (or install Homebrew and run: brew install --cask openrgb)" "No hay Homebrew: instala OpenRGB desde https://openrgb.org (o instala Homebrew y ejecuta: brew install --cask openrgb)")"
    fi
    return 0
  fi
  if [ "$FAMILY" != appimage ] && pm_install_repo openrgb; then
    state_set official native
  elif [ "${1:-}" = revert ]; then
    warn "$(t "The official OpenRGB is not available from your package manager: OpenRGB is now uninstalled." "El OpenRGB oficial no está en los repositorios de tu distribución: OpenRGB queda desinstalado.")"
  else
    # Not packaged for this distribution (Debian and Ubuntu do not ship it): the openrgb-razer
    # builds are upstream OpenRGB plus the Razer fix, and come from the same trusted release page.
    warn "$(t "The official OpenRGB is not available from your package manager. Installing the openrgb-razer build instead (identical to upstream OpenRGB apart from the Razer fix)." "El OpenRGB oficial no está en los repositorios de tu distribución. Instalo en su lugar la build openrgb-razer (idéntica a OpenRGB salvo por el arreglo Razer).")"
    install_openrgb_razer
  fi
}

install_chroma_desk() {
  local tag="${CD_VERSION:-}" urls url re
  if [ -z "$tag" ]; then
    tag="$(latest_tag '^v[0-9]')"
    [ -n "$tag" ] || die "$(t "no chroma-desk release found yet" "todavía no hay ningún release de chroma-desk")"
  fi
  info "$(t "Installing chroma-desk ($tag)" "Instalando chroma-desk ($tag)")"
  urls="$(release_assets "$tag")"
  [ -n "$urls" ] || die "$(t "release $tag has no downloadable files" "el release $tag no tiene archivos")"
  if [ "$OS" = Darwin ]; then
    url="$(pick_asset "$urls" '_universal\.dmg$')" || die "$(t "no macOS build in $tag" "no hay build de macOS en $tag")"
    fetch_verified "$tag" "$url"
    install_dmg "$FETCHED"
    state_set chroma_desk dmg
    return 0
  fi
  re="$(cd_pkg_re || true)"
  if [ "$FAMILY" != appimage ] && [ -n "$re" ] && url="$(pick_asset "$urls" "$re")"; then
    fetch_verified "$tag" "$url"
    pm_install_local "$FETCHED"
    state_set chroma_desk native
  else
    url="$(pick_asset "$urls" "$(cd_appimage_re)")" || die "$(t "no chroma-desk package for $PRETTY ($ARCH) in $tag" "no hay paquete de chroma-desk para $PRETTY ($ARCH) en $tag")"
    fetch_verified "$tag" "$url"
    run mkdir -p "$HOME/.local/bin"
    run install -m 755 "$FETCHED" "$HOME/.local/bin/chroma-desk"
    state_set chroma_desk appimage
    case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) warn "$(t "add ~/.local/bin to your PATH" "añade ~/.local/bin a tu PATH")" ;; esac
  fi
}

install_dmg() { # FILE
  local mnt app
  mk_tmp
  mnt="$TMP/mnt"
  if [ "$DRY_RUN" = 1 ]; then
    printf '  + hdiutil attach %s ; copy chroma-desk.app to /Applications\n' "$1"
    return 0
  fi
  mkdir -p "$mnt"
  run hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$1"
  app="$(find "$mnt" -maxdepth 1 -name '*.app' | head -1)"
  [ -n "$app" ] || {
    hdiutil detach "$mnt" >/dev/null 2>&1 || true
    die "no .app inside the dmg"
  }
  if [ -w /Applications ]; then
    run rm -rf "/Applications/$(basename "$app")"
    run cp -R "$app" /Applications/
  else
    run_sudo rm -rf "/Applications/$(basename "$app")"
    run_sudo cp -R "$app" /Applications/
  fi
  hdiutil detach "$mnt" >/dev/null 2>&1 || true
}

post_install() {
  [ "$OS" = Linux ] || return 0
  if [ "$FAMILY" != appimage ] || [ -e /etc/udev/rules.d/60-openrgb.rules ]; then
    reload_udev
  fi
  if confirm "Load the i2c-dev kernel module now and at every boot (needed by RGB RAM and motherboards)?" "¿Cargar el módulo i2c-dev ahora y en cada arranque (hace falta para RAM y placas RGB)?" y; then
    if run_sudo modprobe i2c-dev; then :; else warn "$(t "modprobe i2c-dev failed (not fatal)" "modprobe i2c-dev falló (no es grave)")"; fi
    mk_tmp
    echo i2c-dev >"$TMP/i2c-dev.conf"
    run_sudo install -m 644 "$TMP/i2c-dev.conf" /etc/modules-load.d/i2c-dev.conf || warn "$(t "could not write /etc/modules-load.d/i2c-dev.conf" "no se pudo escribir /etc/modules-load.d/i2c-dev.conf")"
  fi
  if confirm "Start chroma-desk automatically when you log in (in the tray)?" "¿Iniciar chroma-desk automáticamente al iniciar sesión (en la bandeja)?" n; then
    local f="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/chroma-desk.desktop"
    printf '  + write %s\n' "$f"
    if [ "$DRY_RUN" = 0 ]; then
      mkdir -p "$(dirname "$f")"
      printf '[Desktop Entry]\nType=Application\nName=chroma-desk\nExec=%s --hidden\nTerminal=false\nX-GNOME-Autostart-enabled=true\n' "$(command -v chroma-desk || echo "$HOME/.local/bin/chroma-desk")" >"$f"
    fi
  fi
}

uninstall_all() {
  local razer method cdm
  razer="$(state_get razer)"
  cdm="$(state_get chroma_desk)"
  info "$(t "Removing chroma-desk" "Quitando chroma-desk")"
  if [ "$OS" = Darwin ]; then
    if [ -w /Applications ]; then run rm -rf /Applications/chroma-desk.app; else run_sudo rm -rf /Applications/chroma-desk.app; fi
  elif [ "$cdm" = appimage ] || { [ "$FAMILY" != appimage ] && ! pkg_installed chroma-desk; }; then
    run rm -f "$HOME/.local/bin/chroma-desk"
  else
    pm_remove chroma-desk
  fi
  run rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/chroma-desk.desktop"
  if [ -n "$razer" ]; then
    method="$razer"
    info "$(t "Reverting to the official OpenRGB (openrgb-razer was installed by this script)" "Volviendo al OpenRGB oficial (este script instaló openrgb-razer)")"
    if [ "$method" = appimage ]; then
      run rm -f "$HOME/.local/bin/openrgb"
      [ -e /etc/udev/rules.d/60-openrgb.rules ] && run_sudo rm -f /etc/udev/rules.d/60-openrgb.rules
    elif pkg_installed openrgb-razer || [ "$DRY_RUN" = 1 ]; then
      pm_remove openrgb-razer
    fi
    install_openrgb_official revert
  fi
  state_clear
  info "$(t "Done. Your chroma-desk settings were kept (~/.config/chroma-desk)." "Listo. Tus ajustes de chroma-desk se conservan (~/.config/chroma-desk).")"
}

# ---------------------------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------------------------

info "$(t "System: $PRETTY, $ARCH (package type: $FAMILY)" "Sistema: $PRETTY, $ARCH (tipo de paquete: $FAMILY)")"
[ "$DRY_RUN" = 1 ] && info "$(t "Dry run: nothing will be installed." "Simulación: no se instalará nada.")"

if [ "$UNINSTALL" = 1 ]; then
  confirm "Remove chroma-desk (and revert openrgb-razer if this script installed it)?" "¿Quitar chroma-desk (y revertir openrgb-razer si lo instaló este script)?" y || exit 0
  uninstall_all
  exit 0
fi

USE_RAZER=0
if [ "$OS" = Linux ]; then
  detect_razer
  if [ ${#RAZER_FOUND[@]} -gt 0 ]; then
    info "$(t "Razer devices detected:" "Dispositivos Razer detectados:")"
    printf '    - %s\n' "${RAZER_FOUND[@]}"
  fi
  case "$RAZER" in
    yes) USE_RAZER=1 ;;
    no) USE_RAZER=0 ;;
    auto)
      def=n
      [ ${#RAZER_FOUND[@]} -gt 0 ] && def=y
      if confirm "Do you have Razer devices?" "¿Tienes dispositivos Razer?" "$def"; then USE_RAZER=1; fi
      ;;
  esac
  if [ "$USE_RAZER" = 1 ]; then
    t "openrgb-razer is OpenRGB plus a fix that lets Razer keyboards such as the Huntsman V3 Pro TKL work on Linux
(the official OpenRGB cannot reach their lighting interface). It replaces the official OpenRGB package.
" "openrgb-razer es OpenRGB más un arreglo que hace funcionar en Linux teclados Razer como el Huntsman V3 Pro TKL
(el OpenRGB oficial no llega a su interfaz de iluminación). Sustituye al paquete oficial de OpenRGB.
"
  fi
else
  RAZER=no
  info "$(t "macOS: installing the official OpenRGB (the Razer fix is Linux-only)." "macOS: se instala el OpenRGB oficial (el arreglo Razer es solo para Linux).")"
fi

if [ "$ASSUME_YES" = 0 ] && [ "$DRY_RUN" = 0 ]; then
  confirm "Continue?" "¿Continuar?" y || {
    info "$(t "Cancelled." "Cancelado.")"
    exit 0
  }
fi

if [ "$USE_RAZER" = 1 ]; then install_openrgb_razer; else install_openrgb_official; fi
install_chroma_desk
post_install

cat <<EOF

$(t "Done. Next steps:" "Listo. Siguientes pasos:")
  - $(t "Run: chroma-desk (it starts the OpenRGB server by itself)." "Ejecuta: chroma-desk (arranca el servidor de OpenRGB por sí solo).")
  - $(t "If a device is not found, log out and back in (udev rules / groups) and check 'openrgb --list-devices'." "Si no aparece un dispositivo, cierra sesión y vuelve a entrar (reglas udev / grupos) y prueba 'openrgb --list-devices'.")
  - $(t "Remove everything later with: install.sh --uninstall" "Para quitarlo todo más adelante: install.sh --uninstall")
EOF
