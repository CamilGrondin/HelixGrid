#!/usr/bin/env bash
# ==============================================================================
# sign_and_notarize.sh - Signature et Notarisation Apple Gatekeeper pour HelixGrid
# ==============================================================================
# Prérequis :
# 1. Certificat "Developer ID Application" installé dans le Trousseau d'accès macOS.
# 2. Profil notarytool configuré dans le Trousseau (via xcrun notarytool store-credentials)
#    OU variables d'environnement (APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID).
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_TAURI_DIR="${PROJECT_DIR}/src-tauri"

echo "=========================================================="
echo "  HelixGrid - Signature & Notarisation Apple (macOS)"
echo "=========================================================="

# 1. Vérification du certificat Developer ID Application
DEFAULT_TEAM_ID="Q78YXZ7Z7Z"
IDENTITY="${APPLE_SIGNING_IDENTITY:-}"

if [ -z "$IDENTITY" ]; then
    # Recherche automatique du certificat Developer ID Application
    MATCHING_IDENTITY=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/' || true)
    if [ -n "$MATCHING_IDENTITY" ]; then
        IDENTITY="$MATCHING_IDENTITY"
        echo "[+] Certificat Developer ID détecté : ${IDENTITY}"
    else
        echo "[!] Aucun certificat 'Developer ID Application' trouvé dans le Trousseau d'accès."
        echo "    Les certificats 'Apple Development' ne permettent PAS la distribution publique"
        echo "    et provoqueront l'erreur Gatekeeper : 'HelixGrid est endommagé'."
        echo ""
        # Afficher les certificats existants à titre indicatif
        EXISTING_DEV=$(security find-identity -v -p codesigning | grep "Apple Development" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/' || true)
        if [ -n "$EXISTING_DEV" ]; then
            echo "    Certificat de développement trouvé : ${EXISTING_DEV}"
            echo "    Pour tester en mode développement local uniquement, vous pouvez définir :"
            echo "    export APPLE_SIGNING_IDENTITY=\"${EXISTING_DEV}\""
        fi
        echo ""
        echo "    Veuillez créer et télécharger un certificat 'Developer ID Application' sur :"
        echo "    https://developer.apple.com/account/resources/certificates/add"
        exit 1
    fi
fi

export APPLE_SIGNING_IDENTITY="$IDENTITY"

# 2. Signature des binaires externes (sidecars AutoDock Vina) avec Hardened Runtime et Horodatage
echo ""
echo "[*] Étape 1 : Signature des binaires auxiliaires (sidecars) avec Hardened Runtime..."
for bin_file in "${SRC_TAURI_DIR}"/bin/vina*; do
    if [ -f "$bin_file" ] && [ -x "$bin_file" ]; then
        echo "    -> Signature de $(basename "$bin_file")..."
        codesign --force --timestamp --options runtime \
            --sign "$IDENTITY" \
            "$bin_file"
    fi
done

# 3. Compilation et packaging avec Tauri (y compris artefacts de mise à jour distante)
echo ""
echo "[*] Étape 2 : Compilation du bundle Tauri (.app, .dmg et artefacts d'update .tar.gz/.sig)..."
if [ -f "${HOME}/.helixgrid-secrets/updater.key" ]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(cat "${HOME}/.helixgrid-secrets/updater.key")"
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
    echo "[+] Clé privée de signature Tauri activée (~/.helixgrid-secrets/updater.key)"
elif [ -f "${SRC_TAURI_DIR}/updater.key" ]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(cat "${SRC_TAURI_DIR}/updater.key")"
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
    echo "[+] Clé privée de signature Tauri activée (génération automatique des artefacts d'update signés)"
fi
cd "${PROJECT_DIR}"
npx tauri build --bundles app,dmg

DMG_PATH=$(find "${SRC_TAURI_DIR}/target/release/bundle/dmg" -name "HelixGrid_*.dmg" | head -n 1)
APP_PATH="${SRC_TAURI_DIR}/target/release/bundle/macos/HelixGrid.app"

if [ ! -f "$DMG_PATH" ]; then
    echo "[-] Fichier DMG introuvable dans ${SRC_TAURI_DIR}/target/release/bundle/dmg"
    exit 1
fi
echo "[+] Bundle créé avec succès : ${DMG_PATH}"

UPDATE_TAR=$(find "${SRC_TAURI_DIR}/target/release/bundle/macos" -name "*.tar.gz" 2>/dev/null | head -n 1)
if [ -n "$UPDATE_TAR" ]; then
    echo "[+] Artefact d'update généré : ${UPDATE_TAR}"
    if [ -f "${UPDATE_TAR}.sig" ]; then
        echo "[+] Signature d'update prête : ${UPDATE_TAR}.sig"
    fi
fi

# 4. Notarisation avec xcrun notarytool
echo ""
echo "[*] Étape 3 : Soumission pour Notarisation Apple..."

PROFILE_NAME="${NOTARY_KEYCHAIN_PROFILE:-helixgrid-notary}"

if xcrun notarytool history --keychain-profile "$PROFILE_NAME" >/dev/null 2>&1; then
    echo "    Utilisation du profil Trousseau : ${PROFILE_NAME}"
    xcrun notarytool submit "$DMG_PATH" --keychain-profile "$PROFILE_NAME" --wait
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    echo "    Utilisation des identifiants APPLE_ID / APPLE_PASSWORD..."
    xcrun notarytool submit "$DMG_PATH" \
        --apple-id "$APPLE_ID" \
        --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_PASSWORD" \
        --wait
elif [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ] && [ -n "${APPLE_API_KEY_PATH:-}" ]; then
    echo "    Utilisation de la clé API App Store Connect..."
    xcrun notarytool submit "$DMG_PATH" \
        --key "$APPLE_API_KEY_PATH" \
        --key-id "$APPLE_API_KEY" \
        --issuer "$APPLE_API_ISSUER" \
        --wait
else
    echo "[!] Aucun profil notarytool ni variable d'authentification configurée."
    echo "    Pour configurer votre trousseau une seule fois (recommandé) :"
    echo "    xcrun notarytool store-credentials \"helixgrid-notary\" \\"
    echo "        --apple-id \"votre-email@apple.com\" \\"
    echo "        --team-id \"${DEFAULT_TEAM_ID}\" \\"
    echo "        --password \"mot-de-passe-specifique-d-application\""
    echo ""
    echo "    Puis relancez ce script."
    exit 1
fi

# 5. Agrafage du ticket (Staple)
echo ""
echo "[*] Étape 4 : Agrafage (Stapling) du ticket de notarisation sur le DMG..."
xcrun stapler staple "$DMG_PATH"

if [ -d "$APP_PATH" ]; then
    echo "[*] Agrafage du ticket sur HelixGrid.app..."
    xcrun stapler staple "$APP_PATH" || true
fi

# 6. Validation finale Gatekeeper
echo ""
echo "[*] Étape 5 : Validation finale Gatekeeper..."
xcrun stapler validate "$DMG_PATH"
spctl --assess --type open --context context:primary-signature -v "$DMG_PATH" || true

echo ""
echo "=========================================================="
echo "  [SUCCÈS] HelixGrid est signé, notarisé et agrafé !"
echo "  Le DMG peut être distribué publiquement sans blocage macOS."
echo "=========================================================="
