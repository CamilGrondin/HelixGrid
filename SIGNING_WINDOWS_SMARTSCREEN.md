# Guide de Signature Windows & Réputation Microsoft Defender SmartScreen

Ce guide documente la problématique des blocages **Microsoft Defender SmartScreen** sur Windows et détaille les **solutions concrètes** pour distribuer HelixGrid aux bénévoles sans avertissement de sécurité.

---

## 1. Origine du Problème : Comment fonctionne Microsoft SmartScreen ?

Lorsque les bénévoles téléchargent `HelixGrid-Setup.exe` ou `HelixGrid.msi` sur Windows, le filtre **SmartScreen** analyse le fichier.

Si l'application est :
- **Non signée**, OU
- Signée avec un **certificat standard récent** n'ayant pas encore accumulé suffisamment de téléchargements connus auprès des serveurs Microsoft,

Windows affiche l'écran d'alerte bleu :
> 🛡️ **"Windows a protégé votre ordinateur"**  
> *"Microsoft Defender SmartScreen a empêché le démarrage d'une application non reconnue. L'exécution de cette application peut mettre votre PC en danger."*

Bien que l'application ne soit pas malveillante, ce message effraie souvent les utilisateurs et réduit considérablement l'engagement des bénévoles.

---

## 2. Les 3 Solutions pour Éliminer SmartScreen sur HelixGrid

### Solution 1 (Recommandée & Gratuite pour Open Source) : SignPath Foundation

La **Fondation SignPath** ([signpath.org](https://signpath.org/)) fournit des certificats de signature de code **gratuits et reconnus par SmartScreen** pour les projets open source d'utilité publique hébergés sur GitHub.

#### Étapes de mise en place :
1. Créez un compte sur [SignPath.io](https://about.signpath.io/).
2. Inscrivez le dépôt GitHub `HelixGrid` dans le programme gratuit **Open Source Projects**.
3. Configurez le secret `SIGNPATH_API_TOKEN` dans GitHub Actions.
4. Intégrez l'action GitHub officielle dans le pipeline de build :
   ```yaml
   - name: Sign Windows Installer with SignPath
     uses: signpath/github-action-submit-signing-request@v1
     with:
       api-token: ${{ secrets.SIGNPATH_API_TOKEN }}
       organization-id: 'votre-orga-id'
       project-slug: 'helixgrid'
       signing-policy-slug: 'release-signing'
       artifact-path: 'helixgrid-ui/src-tauri/target/release/bundle/nsis/HelixGrid_*.exe'
   ```
*Résultat : Les binaires signés par SignPath héritent d'une réputation établie, supprimant l'alerte SmartScreen.*

---

### Solution 2 (Cloud Microsoft Officiel à Faible Coût) : Azure Trusted Signing

Anciennement nommé *Microsoft Artifact Signing*, **Azure Trusted Signing** est le service cloud managé de Microsoft spécialement conçu pour remplacer les clés USB physiques HSM coûteuses (~500-800 $/an).

- **Coût** : ~10 $/mois via un abonnement Microsoft Azure.
- **Réputation** : Les binaires signés via Azure Trusted Signing bénéficient d'une réputation SmartScreen immédiate et native.
- **Mise en œuvre** :
  ```powershell
  # Utilisation du dlib Azure SignTool
  azuresigntool sign -du "https://helixgrid.org" -kvu "https://votre-keyvault.vault.azure.net" -kvc "HelixGridCert" HelixGrid.exe
  ```

---

### Solution 3 (Gratuite & Immédiate) : Soumission de Réputation à Microsoft Defender Security Intelligence

Si vous signez l'application avec un certificat standard (ou lors des premières versions non certifiées EV), vous pouvez soumettre chaque nouvelle version directement aux équipes Microsoft Defender pour **enregistrer la réputation du binaire en moins de 2 heures** :

1. Compilez la version finale : `npx tauri build`
2. Rendez-vous sur le portail officiel : [Microsoft Security Intelligence - Submit a file](https://www.microsoft.com/en-us/wdsi/filesubmission)
3. Connectez-vous avec votre compte Microsoft.
4. Sélectionnez **"Software Developer"** puis **"Incorrectly detected as malware / Unknown application"**.
5. Téléversez l'installateur `HelixGrid-Setup.exe` ou l'archive `.zip`.
6. Précisez dans la description :
   > *"HelixGrid is an open-source distributed autonomous computing application for AI drug discovery and non-profit biomedical research."*
7. Microsoft analyse le fichier par sandbox automatisée et valide sa réputation cloud. Dès validation, SmartScreen ne bloque plus le binaire sur les PC des bénévoles.

---

## 3. Script Local de Signature Automatisée

Le script PowerShell [`helixgrid-ui/scripts/sign_windows.ps1`](file:///Users/camilgrondin/Desktop/HelixGrid/helixgrid-ui/scripts/sign_windows.ps1) est prêt à l'emploi.

### Utilisation avec un certificat PFX :
```powershell
$env:SMARTSCREEN_CERT_PATH = "C:\Secrets\helixgrid_codesign.pfx"
$env:SMARTSCREEN_CERT_PASSWORD = "votre_mot_de_passe"

.\helixgrid-ui\scripts\sign_windows.ps1
```

Ce script :
1. Détecte automatiquement `signtool.exe` (Windows SDK 10/11).
2. Signe en SHA-256 avec horodatage RFC 3161 (`http://timestamp.digicert.com`).
3. Signe récursivement les binaires auxiliaires `vina-x86_64-pc-windows-msvc.exe`, `vina.exe` et l'exécutable `HelixGrid.exe`.
4. Signe l'installateur `.msi` et `.exe`.
5. Vérifie l'intégrité de la signature Authenticode.

---

## 4. Instructions pour les Bénévoles (Contournement SmartScreen en 2 Clics)

Dans l'attente de l'établissement complet de la réputation, intégrez ces instructions claires dans le README et la page de téléchargement :

```text
💡 Note pour les utilisateurs Windows :
Lors du premier lancement, Windows Defender SmartScreen peut afficher une alerte bleue :
1. Cliquez sur "Informations complémentaires" (More info).
2. Cliquez sur le bouton "Exécuter quand même" (Run anyway).
HelixGrid est un logiciel open-source certifié sans logiciel espion ni publicité.
```
