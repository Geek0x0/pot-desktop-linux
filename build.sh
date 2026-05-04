#!/usr/bin/env bash
set -euo pipefail

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# ── Helpers ─────────────────────────────────────────────────────────────────
info()  { printf "${BLUE}ℹ${RESET} %s\n" "$*"; }
ok()    { printf "${GREEN}✔${RESET} %s\n" "$*"; }
warn()  { printf "${YELLOW}⚠${RESET} %s\n" "$*"; }
fail()  { printf "${RED}✖${RESET} %s\n" "$*"; }
step()  { printf "\n${BOLD}${CYAN}▸ %s${RESET}\n" "$*"; }

ask_yn() {
    local prompt="$1" default="${2:-Y}"
    local suffix
    [[ "$default" == "Y" ]] && suffix="[Y/n]" || suffix="[y/N]"
    while true; do
        printf "  ${BOLD}%s %s${RESET} " "$prompt" "$suffix"
        read -r answer
        answer="${answer:-$default}"
        case "$answer" in
            [Yy]*) return 0 ;;
            [Nn]*) return 1 ;;
            *) echo "  Please answer y or n." ;;
        esac
    done
}

ask_choice() {
    local prompt="$1"; shift
    local options=("$@")
    local i=1
    printf "  ${BOLD}%s${RESET}\n" "$prompt"
    for opt in "${options[@]}"; do
        printf "    ${DIM}%2d)${RESET} %s\n" "$i" "$opt"
        ((i++))
    done
    while true; do
        printf "  ${BOLD}Enter choice [1-%d]:${RESET} " "$((i-1))"
        read -r choice
        if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice < i )); then
            SELECTED="${options[$((choice-1))]}"
            return 0
        fi
        echo "  Invalid choice."
    done
}

separator() { printf "\n${DIM}──────────────────────────────────────────────────${RESET}\n"; }

# ── Project paths ───────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# ── Feature selection ───────────────────────────────────────────────────────
declare -A FEATURE_ENABLED
FEATURE_ENABLED[tts]=1
FEATURE_ENABLED[ocr]=1
FEATURE_ENABLED[hotkey]=1
FEATURE_ENABLED[plugin]=0
FEATURE_ENABLED[tray]=1

select_features() {
    step "Feature selection"

    echo "  Available features (all deps available in Docker):"
    echo "    tts     — Text-to-speech via GStreamer"
    echo "    ocr     — OCR with language detection"
    echo "    hotkey  — Global shortcuts (X11 + Wayland)"
    echo "    plugin  — JavaScript plugin runtime"
    echo "    tray    — System tray icon"
    echo ""

    if ask_yn "Enable TTS (text-to-speech)?" "Y"; then FEATURE_ENABLED[tts]=1; else FEATURE_ENABLED[tts]=0; fi
    if ask_yn "Enable OCR with language detection?" "Y"; then FEATURE_ENABLED[ocr]=1; else FEATURE_ENABLED[ocr]=0; fi
    if ask_yn "Enable global hotkeys?" "Y"; then FEATURE_ENABLED[hotkey]=1; else FEATURE_ENABLED[hotkey]=0; fi
    if ask_yn "Enable JavaScript plugin runtime?" "N"; then FEATURE_ENABLED[plugin]=1; else FEATURE_ENABLED[plugin]=0; fi
    if ask_yn "Enable system tray icon?" "Y"; then FEATURE_ENABLED[tray]=1; else FEATURE_ENABLED[tray]=0; fi
}

# ── Locale compilation ─────────────────────────────────────────────────────
COMPILE_LOCALES=1
SELECTED_LOCALES=()

ALL_LOCALES=()
if [[ -d "data/po" ]]; then
    for po in data/po/*.po; do
        [[ -f "$po" ]] && ALL_LOCALES+=("$(basename "$po" .po)")
    done
fi

select_locales() {
    if [[ ${#ALL_LOCALES[@]} -eq 0 ]]; then
        warn "No .po files found in data/po/"
        COMPILE_LOCALES=0
        return
    fi

    if ! ask_yn "Compile locale files (.po → .mo)?" "Y"; then
        COMPILE_LOCALES=0
        return
    fi

    step "Locale selection"
    echo "  Available locales: ${ALL_LOCALES[*]}"

    if ask_yn "Compile all locales?" "Y"; then
        SELECTED_LOCALES=("${ALL_LOCALES[@]}")
    else
        SELECTED_LOCALES=()
        for loc in "${ALL_LOCALES[@]}"; do
            if ask_yn "Include locale '$loc'?" "Y"; then
                SELECTED_LOCALES+=("$loc")
            fi
        done
    fi

    if [[ ${#SELECTED_LOCALES[@]} -eq 0 ]]; then
        warn "No locales selected — skipping"
        COMPILE_LOCALES=0
    fi
}

# ── Package selection ──────────────────────────────────────────────────────
PACKAGE_MODE="binary"

select_package() {
    step "Packaging"

    local options=("binary — executable only" "deb — Debian/Ubuntu package (.deb)" "rpm — Fedora/RHEL package (.rpm)" "appimage — portable AppImage")
    ask_choice "Select output format:" "${options[@]}"

    case "$SELECTED" in
        binary*)  PACKAGE_MODE="binary" ;;
        deb*)     PACKAGE_MODE="deb" ;;
        rpm*)     PACKAGE_MODE="rpm" ;;
        appimage*) PACKAGE_MODE="appimage" ;;
    esac
}

# ── Summary ────────────────────────────────────────────────────────────────
show_summary() {
    separator
    step "Build summary"

    echo ""
    printf "  %-22s %s\n" "Mode:" "Docker"
    printf "  %-22s %s\n" "Output:" "$PACKAGE_MODE"

    local features=()
    for feat in tts ocr hotkey plugin tray; do
        if [[ "${FEATURE_ENABLED[$feat]}" == "1" ]]; then
            features+=("$feat")
        fi
    done
    if [[ ${#features[@]} -gt 0 ]]; then
        printf "  %-22s %s\n" "Features:" "${features[*]}"
    else
        printf "  %-22s %s\n" "Features:" "(none — minimal build)"
    fi

    if [[ "$COMPILE_LOCALES" == "1" ]] && [[ ${#SELECTED_LOCALES[@]} -gt 0 ]]; then
        printf "  %-22s %s\n" "Locales:" "${SELECTED_LOCALES[*]}"
    else
        printf "  %-22s %s\n" "Locales:" "skipped"
    fi

    echo ""
}

# ── Docker build ───────────────────────────────────────────────────────────
docker_build_features() {
    local enabled=()
    for feat in tts ocr hotkey plugin tray; do
        if [[ "${FEATURE_ENABLED[$feat]}" == "1" ]]; then
            enabled+=("$feat")
        fi
    done

    if [[ ${#enabled[@]} -eq 0 ]]; then
        echo ""
        return
    fi

    # Check if all default features are enabled without extras
    local defaults="tts ocr hotkey"
    local all_defaults=1
    for d in tts ocr hotkey; do
        if [[ "${FEATURE_ENABLED[$d]}" != "1" ]]; then
            all_defaults=0
            break
        fi
    done

    local has_extra=0
    for e in plugin tray; do
        if [[ "${FEATURE_ENABLED[$e]}" == "1" ]]; then
            has_extra=1
            break
        fi
    done

    if [[ "$all_defaults" == "1" ]] && [[ "$has_extra" == "0" ]]; then
        echo ""
    else
        echo "${enabled[*]}" | tr ' ' ','
    fi
}

do_docker_build() {
    step "Docker build"

    # Verify Docker is available
    if ! command -v docker &>/dev/null; then
        fail "Docker is not installed"
        exit 1
    fi
    if ! docker info &>/dev/null 2>&1; then
        fail "Docker daemon is not running"
        exit 1
    fi

    local features
    features=$(docker_build_features)
    local service="build"

    if [[ "$PACKAGE_MODE" == "deb" ]]; then
        service="deb"
    elif [[ "$PACKAGE_MODE" == "rpm" ]]; then
        service="rpm"
    elif [[ "$PACKAGE_MODE" == "appimage" ]]; then
        service="appimage"
    fi

    mkdir -p output

    info "Features: ${features:-(default)}"
    info "Service:  $service"
    echo ""

    if FEATURES="${features}" docker compose run --rm --build "$service"; then
        if [[ "$PACKAGE_MODE" == "deb" ]] && ls output/*.deb 1>/dev/null 2>&1; then
            local pkg_file
            pkg_file=$(ls -t output/*.deb | head -1)
            local pkg_size
            pkg_size=$(du -h "$pkg_file" | cut -f1)
            ok "Package: $pkg_file ($pkg_size)"
        elif [[ "$PACKAGE_MODE" == "rpm" ]] && ls output/*.rpm 1>/dev/null 2>&1; then
            local pkg_file
            pkg_file=$(ls -t output/*.rpm | head -1)
            local pkg_size
            pkg_size=$(du -h "$pkg_file" | cut -f1)
            ok "Package: $pkg_file ($pkg_size)"
        elif [[ "$PACKAGE_MODE" == "appimage" ]] && ls output/*.AppImage 1>/dev/null 2>&1; then
            local pkg_file
            pkg_file=$(ls -t output/*.AppImage | head -1)
            local pkg_size
            pkg_size=$(du -h "$pkg_file" | cut -f1)
            ok "Package: $pkg_file ($pkg_size)"
        elif [[ -f output/pot-gtk ]]; then
            local bin_size
            bin_size=$(du -h output/pot-gtk | cut -f1)
            ok "Binary: output/pot-gtk ($bin_size)"
        fi
    else
        fail "Docker build failed"
        exit 1
    fi
}

# ── Main ───────────────────────────────────────────────────────────────────
main() {
    printf "\n${BOLD}${CYAN}╔══════════════════════════════════════╗${RESET}\n"
    printf "${BOLD}${CYAN}║     Pot GTK — Docker Build Script    ║${RESET}\n"
    printf "${BOLD}${CYAN}╚══════════════════════════════════════╝${RESET}\n"

    select_features
    select_locales
    select_package
    show_summary

    if ! ask_yn "Start build?" "Y"; then
        info "Aborted."
        exit 0
    fi

    do_docker_build

    separator
    ok "All done!"
    echo ""
}

main "$@"
