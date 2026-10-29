<p align="center">
  <img src="docs/assets/fontcatch-banner.png" alt="Fontcatch — gestionnaire de polices de bureau" width="960">
</p>

# Fontcatch

**Télécharge une typo. Fontcatch s’occupe de l’installation.**

Une petite app de bureau pour les designers qui préfèrent essayer une police plutôt que fouiller son ZIP. Fontcatch surveille tes téléchargements, repère les fichiers de police et les installe pour toi. Tes typos, leurs aperçus et l’historique restent réunis dans un panneau accessible depuis la barre de menu ou la zone de notification.

[English](README.md) · [Comment ça marche](#comment-ça-marche) · [Commencer](#commencer) · [Contribuer](CONTRIBUTING.md) · [Signaler un problème](https://github.com/kooler98/FontCatch/issues) · [Licence MIT](LICENSE)

## Pourquoi Fontcatch ?

Tu trouves la bonne typo, tu la télécharges, puis les manipulations commencent. Décompresser le ZIP. Retrouver le dossier extrait. Ouvrir un sous-dossier. Trier les OTF, TTF, versions variables, polices web, README et documents de licence. Repérer la version voulue. L’installer. Recommencer pour la famille suivante.

Fontcatch prend en charge les étapes d’installation pour te laisser comparer les typos et avancer sur ton design. L’app s’adresse aux designers de marque, graphistes, designers UI, directeurs artistiques et à toute personne qui essaie régulièrement de nouvelles polices. Les développeurs front-end et designers qui préparent les polices d’un site peuvent aussi convertir leurs fichiers locaux en WOFF2.

## Comment ça marche

1. **Laisse Fontcatch ouvert.** L’app surveille Téléchargements par défaut. Tu peux ajouter d’autres dossiers dans les réglages.
2. **Télécharge ta typo comme d’habitude.** Les archives ZIP et les fichiers `.ttf`, `.otf` et `.ttc` sont pris en charge. Pas besoin de décompresser l’archive ni de retrouver son dossier de polices.
3. **Laisse Fontcatch l’installer.** Le mode Automatique est activé par défaut. Si une famille propose des alternatives variables et statiques, Fontcatch privilégie la version variable ; choisis Statique ou Les deux dans les réglages si tu préfères. Le mode Demander permet de valider chaque installation.
4. **Ouvre ta bibliothèque.** Visualise la police installée, essaie un autre style, saisis ton propre texte, règle sa taille ou copie son nom pour la retrouver dans tes outils.

Fontcatch surveille les **nouveaux téléchargements pendant qu’il est ouvert**. Il n’installe pas les anciens fichiers déjà présents au démarrage ou lors de l’ajout d’un dossier. Les demandes déjà enregistrées sont reprises en mode Automatique. Les mises à jour d’une police installée restent à confirmer.

Les ZIP pris en charge sont les archives ZIP32 classiques, sans compression ou avec compression Deflate, dans les limites de taille et de métadonnées. Les archives ZIP64, fractionnées ou chiffrées ne sont pas prises en charge. Les polices web (`.woff` / `.woff2`) ne peuvent pas être installées.

## Ce que tu peux faire

| | |
| --- | --- |
| **Installer depuis un ZIP** | Repérer et installer les polices de bureau sans extraire manuellement l’archive. Les ZIP sans rapport sont ignorés. |
| **Choisir les bonnes versions** | Privilégier les polices variables, statiques ou les deux lorsqu’une famille propose plusieurs alternatives. Les doublons de noms PostScript sont vérifiés avant installation. |
| **Essayer la vraie typo** | Afficher la police installée, changer de style, saisir ton texte et ajuster la taille. |
| **Exporter une police web** | Enregistrer le style sélectionné en WOFF2 ou convertir un TTF/OTF local sans l’installer. |
| **Retrouver une police** | Chercher dans la bibliothèque Fontcatch par famille, fonderie ou source ; filtrer par type et par provenance lorsqu’elle est disponible. |
| **Consulter les informations utiles** | Retrouver les styles, versions, sources et licences intégrées, ou afficher directement le fichier installé. |
| **Retirer une famille** | Vérifier les fichiers concernés, puis déplacer les polices gérées vers la Corbeille. Une collection TTC partagée indique toutes les familles affectées. |
| **Choisir le fonctionnement** | Mode Automatique ou Demander, notifications, lancement au démarrage et nettoyage du ZIP après une installation réussie en option. |

**À propos des licences :** Fontcatch affiche les informations de licence intégrées au fichier de police. Il ne lit ni n’interprète les README, PDF ou documents de licence séparés présents dans le téléchargement. Si la police ne contient pas ces informations, l’app l’indique. Conserve la licence fournie par la fonderie : Fontcatch ne détermine pas les usages qu’elle autorise.

La bibliothèque contient les polices installées avec Fontcatch. Elle ne recense pas toutes les polices déjà présentes sur ton ordinateur.

### Exporter en WOFF2

Disponible dans la **version de test Windows 0.4.0** et les builds construits depuis les sources.

- **Depuis la bibliothèque :** sélectionne un style, puis **Exporter en WOFF2**. Pour une collection TTC, seule la face sélectionnée est exportée.
- **Depuis un fichier :** choisis **Convertir un fichier…**, puis un `.ttf` ou `.otf`. La police est convertie sans être installée ni ajoutée à la bibliothèque.

Choisis un nouveau nom et un emplacement dans la fenêtre d’enregistrement du système. La conversion reste locale, une police à la fois, avec une limite de 32 Mio en entrée. Les originaux restent inchangés et aucun fichier existant n’est écrasé. L’export conserve les métadonnées de licence intégrées ; vérifie que la licence autorise la conversion et l’utilisation sur le web. WOFF2 est ici un format d’export, pas un format à installer.

## Un fonctionnement local

Fontcatch ne demande pas de compte, n’envoie pas ta bibliothèque dans le cloud, ne collecte pas de statistiques d’utilisation et ne télécharge aucune police à ta place. L’app travaille avec les fichiers que tu choisis de télécharger.

- Installation pour l’utilisateur courant, sans droits administrateur.
- Vérification des archives et des fichiers, avec des limites de taille et d’extraction. Les chemins d’archive dangereux et les liens symboliques sont rejetés.
- Préservation des polices installées en dehors de Fontcatch. Les mises à jour demandent une confirmation, même en mode Automatique.
- Confirmation de la désinstallation et vérification des fichiers gérés avant leur déplacement vers la Corbeille.
- Conservation des téléchargements d’origine par défaut. Le nettoyage des ZIP est optionnel et suit uniquement une installation réussie.

L’app utilise Tauri et Rust, avec le moteur web du système plutôt qu’un navigateur embarqué. Son panneau mesure 360 × 480. L’anglais est la langue par défaut ; le français reste disponible dans les réglages.

Le [compte rendu de sécurité et de performance](docs/security-and-performance.md) détaille les contrôles effectués et les limites connues. Consulter [SECURITY.md](SECURITY.md) pour signaler une vulnérabilité en privé. Fontcatch est une première version alpha ; les validations locales ne remplacent pas un audit de sécurité indépendant.

## Commencer

**Version actuelle : 0.4.0 alpha.**

**[Télécharger Fontcatch pour Windows x64](https://github.com/kooler98/FontCatch/releases/download/v0.4.0/Fontcatch_0.4.0_x64-setup.exe)** · [Notes de version et empreintes SHA-256](https://github.com/kooler98/FontCatch/releases/tag/v0.4.0)

Pour **Windows 10/11 · x64**, télécharge l’installeur `.exe` ci-dessus, lance-le, puis ouvre Fontcatch depuis le menu Démarrer. L’installation concerne ton compte ; si WebView2 manque, l’installeur le télécharge.

**macOS · Apple Silicon :** construis l’app depuis ses sources avec les instructions ci-dessous. Aucun binaire macOS n’est disponible au téléchargement.

Laisse Fontcatch ouvert et télécharge une nouvelle police. Son icône dans la barre de menu ou la zone de notification permet d’ouvrir la bibliothèque et les réglages. Fermer le panneau laisse la détection active ; choisis **Quit Fontcatch** dans le menu de l’icône pour quitter l’app.

L’installeur de test Windows n’est pas signé. Les builds macOS n’ont ni signature de distribution ni notarisation. Ton système peut afficher un avertissement ou bloquer leur ouverture.

La version de test Windows 0.4.0 a passé les vérifications automatisées. Les tests manuels de l’interface sous Windows 10/11 restent à faire.

| Plateforme | Validation effectuée |
| --- | --- |
| macOS · Apple Silicon | Build local 0.4.0 testé avant le correctif de retrait Windows : export WOFF2 d’un style installé et d’un fichier externe via les fenêtres natives, décodage indépendant, originaux conservés et annulation de l’enregistrement. La bibliothèque est restée utilisable pendant l’attente d’accès initial au dossier ; après autorisation macOS, l’avertissement a disparu et la surveillance a démarré. Aucun nouveau téléchargement réel n’a encore été testé en 0.4.0 ; les anciens tests d’installation, d’aperçu et de retrait concernent la 0.3.4. |
| Windows 10/11 · x64 visé | Version 0.4.0 : [Vérifications Windows réussies](https://github.com/kooler98/FontCatch/actions/runs/40904840143) : 99 tests Rust, un test natif d’installation et de retrait de police, création de l’installeur NSIS et démarrage de l’app. La validation manuelle de l’interface reste à faire. |
| macOS · Intel | Pas encore testé ; aucune version Intel à télécharger. |
| Linux | Expérimental. Fonctionnement natif non validé ; une alerte connue sur une dépendance GTK/glib doit être résolue avant distribution. |

La machine de test Windows utilise Windows Server 2022 x64 avec un compte administrateur. Cela ne remplace pas un test de l’interface sous Windows 10/11 avec un compte standard. Ces vérifications manuelles restent à faire, notamment la zone de notification, la mise à l’échelle de l’affichage et l’installation sans élévation des droits.

### Lancer depuis les sources

Installer Node.js 22.12+ (testé avec 24), Rust stable à jour et les [prérequis Tauri de son système](https://v2.tauri.app/start/prerequisites/). Cloner ensuite le projet et le lancer :

```sh
git clone https://github.com/kooler98/FontCatch.git
cd FontCatch
npm ci
npm run desktop
```

Construire une application locale :

```sh
npm run desktop:build
# Sur macOS, construire uniquement l’application :
npm run desktop:build -- --bundles app
```

Les builds locaux ne demandent pas de certificat de distribution. Le dossier optionnel `.tooling/` peut fournir une installation locale de Rust ; sur macOS, `.tooling/sdk-path` permet de choisir un SDK installé. Ces fichiers ne sont pas inclus dans le dépôt.

### Travailler sur l’interface

```sh
npm run dev
```

Ouvrir <http://127.0.0.1:1420>. Le mode navigateur utilise des données d’exemple identifiées et des réglages séparés. Il ne peut ni installer de polices ni accéder à la bibliothèque native.

### Lancer les vérifications

```sh
npm run build
npm test
npm run test:rust
npm run test:tooling
npm run test:ui
```

Les tests Rust ordinaires utilisent des dossiers temporaires et une installation simulée. Les tests navigateur couvrent les aperçus, filtres, réglages, licences et confirmations de désinstallation. Ils utilisent Google Chrome lorsqu’il est disponible sur macOS, sinon Playwright Chromium (`npx playwright install chromium`). La variable `PLAYWRIGHT_CHROMIUM_EXECUTABLE` permet de choisir un autre exécutable.

Des tests macOS et Windows séparés, désactivés par défaut, installent une police synthétique originale dans le vrai dossier de polices de l’utilisateur, vérifient son fonctionnement natif, puis la déplacent vers la Corbeille. Lire [CONTRIBUTING.md](CONTRIBUTING.md) avant de les exécuter.

## Données et structure du projet

Les réglages utilisent Tauri Store ; les polices et demandes en attente utilisent SQLite. Sur macOS, les données de l’app se trouvent dans `~/Library/Application Support/app.fontcatch.desktop/`. Des copies privées permettent de traiter une demande en attente même si le téléchargement d’origine a été déplacé. Les fichiers installés vont dans le dossier de polices de l’utilisateur courant.

| Élément | Emplacement |
| --- | --- |
| Écrans et mise en page | `src/App.tsx`, `src/styles/app.css` |
| Couleurs et typographie | `src/styles/tokens.css` |
| Composants d’interface | `src/components/beautiful-ui/` |
| Textes anglais et français | `src/locales/` |
| Liens du site, du soutien et du créateur | `src/config.json` |
| Identité visuelle | `src/assets/brand/`, `docs/assets/` |
| Détection, lecture et installation | `src-tauri/src/` |

Les liens du site et du soutien restent absents tant qu’ils ne sont pas configurés. Renseigner des URL HTTPS dans `websiteUrl` et `coffeeUrl` active les boutons correspondants. `npm run icons` régénère les icônes de l’app et de la barre de menu à partir des visuels fournis.

## La suite

Tests manuels sous Windows 10/11, validation native de Linux et macOS Intel, résolution des alertes de dépendances Linux, installeurs signés et mises à jour automatiques de l’app. Les tests sur d’autres systèmes et les signalements de bugs précis sont les bienvenus : voir [Contribuer](CONTRIBUTING.md).

## Licence et crédits

Fontcatch est open source sous [licence MIT](LICENSE). Les polices téléchargées conservent leurs propres licences ; celle de Fontcatch ne donne aucun droit supplémentaire sur leur utilisation.

Certains composants sont adaptés de Beautiful UI, créé par Shane Levine, avec leur mention MIT conservée. Les polices synthétiques originales de test utilisent CC0. Voir [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

<p align="center">
  <img src="docs/assets/fontcatch-wordmark.png" alt="Fontcatch" width="172"><br>
  Made by <a href="https://twitter.com/pierrreweb">@pierrreweb</a>
</p>
