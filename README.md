# HelixGrid Desktop — Client Bénévole Distribué

Application de bureau autonome pour le calcul distribué de criblage virtuel moléculaire par IA (AutoDock Vina, GNN D-MPNN, modèles ONNX).

---

## 🚀 Plateformes Supportées & Binaires AutoDock Vina

L'application intègre les binaires natifs **AutoDock Vina v1.2.7** pour chaque architecture cible sous `src-tauri/bin/` :

| Plateforme | Architecture | Binaire Tauri Sidecar | Statut |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Silicon (`aarch64-apple-darwin`) | `vina-aarch64-apple-darwin` | ✅ Inclus |
| **macOS** | Intel x64 (`x86_64-apple-darwin`) | `vina-x86_64-apple-darwin` | ✅ Inclus |
| **Windows** | x86_64 (`x86_64-pc-windows-msvc`) | `vina-x86_64-pc-windows-msvc.exe` | ✅ Inclus |
| **Linux** | x86_64 (`x86_64-unknown-linux-gnu`) | `vina-x86_64-unknown-linux-gnu` | ✅ Inclus |
| **Linux** | ARM64 (`aarch64-unknown-linux-gnu`) | `vina-aarch64-unknown-linux-gnu` | ✅ Inclus |

Pour re-télécharger ou mettre à jour les binaires Vina :
```bash
python scripts/fetch_vina_binaries.py
```

---

## 🔐 Signature de Code & Sécurité des Systèmes d'Exploitation

### 🍏 macOS (Gatekeeper & Notarisation Apple)
Pour éviter l'erreur *"HelixGrid est endommagé et ne peut pas être ouvert"* :
- Certificat requis : **Developer ID Application** (avec Hardened Runtime et horodatage).
- Notarisation : automatisée via `xcrun notarytool` et agrafage du ticket (*stapling*).
- Script complet : [`scripts/sign_and_notarize.sh`](file:///Users/camilgrondin/Desktop/HelixGrid/helixgrid-ui/scripts/sign_and_notarize.sh).
- Guide détaillé : [NOTARIZATION_MACOS.md](file:///Users/camilgrondin/Desktop/HelixGrid/helixgrid-ui/NOTARIZATION_MACOS.md).

### 🪟 Windows (Microsoft Defender SmartScreen)
Pour éviter l'alerte bleue *"Windows a protégé votre ordinateur (application non reconnue)"* :
- Signature Authenticode : automatisée avec `signtool.exe` en SHA-256 avec horodatage RFC 3161 via [`scripts/sign_windows.ps1`](file:///Users/camilgrondin/Desktop/HelixGrid/helixgrid-ui/scripts/sign_windows.ps1).
- Établissement de réputation SmartScreen :
  1. **Gratuit pour Open Source :** [SignPath.io Foundation](https://signpath.org/) (recommandé).
  2. **Cloud Microsoft :** Azure Trusted Signing (~10$/mois).
  3. **Soumission de réputation :** Téléversement gratuit du binaire sur le portail [Microsoft Security Intelligence File Submission](https://www.microsoft.com/en-us/wdsi/filesubmission).
- Guide complet : [SIGNING_WINDOWS_SMARTSCREEN.md](file:///Users/camilgrondin/Desktop/HelixGrid/helixgrid-ui/SIGNING_WINDOWS_SMARTSCREEN.md).

---

## 🛠️ Développement Local

```bash
# Installer les dépendances
npm install

# Lancer en mode développement
npm run tauri dev

# Compiler pour la production
npm run tauri build
```
