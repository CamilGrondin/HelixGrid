# Guide de Signature et Notarisation Apple (macOS Gatekeeper)

Ce guide décrit la procédure pas à pas pour signer et notariser **HelixGrid** avec un certificat **Apple Developer ID** et **`xcrun notarytool`**, afin d'éliminer définitivement l'erreur bloquante de macOS :
> **"HelixGrid est endommagé et ne peut pas être ouvert. Vous devriez le placer dans la Corbeille."**

---

## 1. Origine du Problème

Lorsqu'un utilisateur télécharge une application depuis Internet (GitHub Releases, navigateur, etc.), macOS lui attribue automatiquement l'attribut étendu `com.apple.quarantine`.

Au lancement, **macOS Gatekeeper** vérifie :
1. **La signature de code** : elle doit obligatoirement être issue d'un certificat officiel **Developer ID Application** (un certificat `Apple Development` est rejeté hors de votre Mac personnel).
2. **Le Hardened Runtime** : l'exécutable et ses binaires auxiliaires (`bin/vina`) doivent être compilés avec l'option runtime sécurisée.
3. **La Notarisation Apple** : les binaires doivent avoir été scannés et approuvés par les serveurs d'Apple via `notarytool`, et le ticket doit être agrafé (*stapled*) au fichier `.dmg` ou `.app`.

Si l'une de ces conditions manque, Gatekeeper bloque l'ouverture avec le message "l'application est endommagée".

---

## 2. Étape 1 : Obtenir le certificat Developer ID Application

1. Connectez-vous sur [Apple Developer - Certificates](https://developer.apple.com/account/resources/certificates/list).
2. Cliquez sur le bouton **+** pour ajouter un certificat.
3. Sélectionnez impérativement : **Developer ID Application** *(ne pas choisir "Apple Development" ni "Mac App Distribution")*.
4. Suivez les instructions pour générer une demande de signature de certificat (CSR) via *Trousseaux d'accès* > *Assistant de certification*.
5. Téléchargez le fichier `.cer` et double-cliquez dessus pour l'importer dans votre trousseau de session.
6. Vérifiez sa présence dans le terminal :
   ```bash
   security find-identity -v -p codesigning
   ```
   Vous devez voir apparaître :
   ```text
   "Developer ID Application: Camil GRONDIN (U4SSQ985J3)"
   ```

---

## 3. Étape 2 : Configurer les identifiants pour `xcrun notarytool`

`notarytool` remplace l'ancien outil `altool`. Il permet d'enregistrer vos identifiants dans le Trousseau de manière sécurisée sous un profil nommé `helixgrid-notary`.

### Option A (Recommandée) : Clé API App Store Connect
1. Rendez-vous sur [App Store Connect > Utilisateurs et accès > Clés](https://appstoreconnect.apple.com/access/integrations/api).
2. Générez une clé avec le rôle **Développeur**.
3. Téléchargez la clé privée `.p8` (ex: `AuthKey_ABCD1234EF.p8`), notez l'**Issuer ID** et le **Key ID**.
4. Enregistrez les identifiants dans votre trousseau local :
   ```bash
   xcrun notarytool store-credentials "helixgrid-notary" \
     --key "/chemin/vers/AuthKey_ABCD1234EF.p8" \
     --key-id "ABCD1234EF" \
     --issuer "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
   ```

### Option B : Mot de passe spécifique d'application
1. Rendez-vous sur [appleid.apple.com](https://appleid.apple.com) > *Sécurité du compte* > *Mots de passe pour applications*.
2. Générez un mot de passe intitulé `HelixGrid-Notarize`.
3. Enregistrez les identifiants dans votre trousseau :
   ```bash
   xcrun notarytool store-credentials "helixgrid-notary" \
     --apple-id "votre-email@apple.com" \
     --team-id "U4SSQ985J3" \
     --password "xxxx-xxxx-xxxx-xxxx"
   ```

---

## 4. Étape 3 : Configuration du Projet (Déjà intégrée)

Les éléments suivants sont déjà configurés dans le dépôt :
* **`helixgrid-ui/src-tauri/Entitlements.plist`** : définit les permissions JIT et chargement dynamique d'ONNX Runtime en mode Hardened Runtime.
* **`helixgrid-ui/src-tauri/tauri.conf.json`** : active `macOS.hardenedRuntime` et référence le fichier d'entitlements.
* **`helixgrid-ui/scripts/sign_and_notarize.sh`** : script automatisé pour signer les sidecars (`bin/vina`), lancer le build Tauri, soumettre à Apple, agrafer le ticket et valider Gatekeeper.

---

## 5. Étape 4 : Lancer la Signature et la Notarisation

Depuis le dossier `helixgrid-ui` :

```bash
cd helixgrid-ui
npm run notarize
```

Le script effectue automatiquement les 5 étapes suivantes :
1. Détection du certificat `Developer ID Application: Camil GRONDIN (U4SSQ985J3)`.
2. Signature cryptographique des binaires auxiliaires `vina` avec l'option `--options runtime --timestamp`.
3. Compilation du projet et création du DMG via `tauri build`.
4. Envoi et notarisation auprès des serveurs Apple via `xcrun notarytool submit ... --wait`.
5. Agrafage (*stapling*) du ticket de notarisation sur le fichier `.dmg` et l'application `.app`.

---

## 6. Vérification Locale et Test Gatekeeper

Pour tester localement comme si le fichier venait d'être téléchargé par un utilisateur :

1. **Vérifier l'agrafage** :
   ```bash
   xcrun stapler validate src-tauri/target/release/bundle/dmg/HelixGrid_0.3.0_aarch64.dmg
   ```
   *(Doit afficher : `The validate action worked!`)*

2. **Vérifier l'acceptation Gatekeeper** :
   ```bash
   spctl -a -t open --context context:primary-signature -v src-tauri/target/release/bundle/dmg/HelixGrid_0.3.0_aarch64.dmg
   ```
   *(Doit afficher : `accepted / source=Notarized Developer ID`)*

3. **Tester avec la quarantaine simulée** :
   ```bash
   xattr -w com.apple.quarantine "0081;63b9f0d1;Chrome;..." src-tauri/target/release/bundle/dmg/HelixGrid_0.3.0_aarch64.dmg
   open src-tauri/target/release/bundle/dmg/HelixGrid_0.3.0_aarch64.dmg
   ```
   L'application s'ouvrira immédiatement avec le message rassurant d'Apple :
   *"HelixGrid a été vérifié par Apple contre les logiciels malveillants."*
