# EpiKodi

Mediacenter minimal en Rust (Tauri 2) et React : connexion à des partages SMB/Samba, liste des vidéos sans miniature, lecture dans une fenêtre mpv séparée.

## Prérequis

### Windows

- Rust (toolchain MSVC), Node.js, Visual Studio Build Tools (outils C++ / MSVC), WebView2.
- `libmpv/` doit être préparé avant tout build, avec `scripts/setup-libmpv.ps1`. Le script télécharge la dernière release `mpv-dev-x86_64` de [shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake) depuis GitHub et crée `libmpv/` (`libmpv-2.dll`, en-têtes, `mpv.lib`). Il est idempotent. Il a besoin d'Internet et des outils MSVC (localisés via `vswhere`).

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup-libmpv.ps1
```

Sans `libmpv/`, `build.rs` échoue avec le message « lancer scripts/setup-libmpv.ps1 ».

### Linux (Debian/Ubuntu)

```sh
sudo apt install libmpv-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev build-essential pkg-config
```

La compilation a été vérifiée sur **Debian 13 (libmpv 0.40)**. L'application n'a **jamais été lancée avec une interface graphique** sous Linux. Les versions plus anciennes de libmpv ne sont pas testées.

## Lancement

```sh
npm install
npx tauri dev
```

Build de débogage sans bundle :

```sh
npx tauri build --debug --no-bundle
```

Vérification du frontend seul : `npm run build` (`tsc` puis `vite build`).

## Utilisation

1. **Hôte** : adresse du serveur, par exemple `192.168.1.10`, ou `hote:port` (par exemple `127.0.0.1:1445`).
2. **Partage** : nom exact du partage. Il n'y a pas de listing des partages du serveur.
3. **Utilisateur** et **Mot de passe** : identifiants SMB.
4. **Connecter** : la connexion apparaît dans « Partages connectés ». La sélectionner pour naviguer.
5. Un clic sur un dossier l'ouvre. Le fil d'Ariane permet de remonter. Un clic sur une vidéo la lance dans une fenêtre mpv.
6. Contrôles : la barre OSC de mpv à la souris, et les raccourcis clavier par défaut de mpv (par exemple espace pour pause, flèches pour se déplacer). La fenêtre mpv se ferme à la fin de la vidéo ; le clic suivant en recrée une.

## Samba de test avec Docker

Sous Windows, le port 445 est occupé : le test utilise le port 1445. Partage `Shared`, hôte `127.0.0.1:1445`, utilisateur `epi`, mot de passe `epipass` (identifiants de test uniquement).

```sh
docker run -d --name epikodi-test-samba -p 1445:445 -e USER=epi -e PASS=epipass -v "<dossier_partage>:/storage" dockurr/samba
```

Vidéo de test (60 s, 1280x720), à déposer dans le dossier partagé :

```sh
docker run --rm -v "<dossier_partage>:/out" jrottenberg/ffmpeg:latest -y -f lavfi -i testsrc=duration=60:size=1280x720:rate=30 -f lavfi -i sine=frequency=440:duration=60 -c:v libx264 -b:v 2M -c:a aac -pix_fmt yuv420p -shortest "/out/video-test.mp4"
```

## Limites connues

- Les connexions ne sont pas persistées : il faut se reconnecter à chaque lancement. Le mot de passe n'est jamais écrit sur disque.
- Pas de découverte réseau ni de listing des partages d'un serveur : le nom du partage doit être saisi.
- Les messages d'erreur SMB sont en anglais (statuts NTSTATUS).
- Le libellé d'une connexion n'affiche pas le port.
- Le proxy local sert tout fichier du partage aux détenteurs du token de session, pas seulement les vidéos. Le filtre par extension ne s'applique qu'à la liste.
- Testé avec des fichiers de 6 Mo. Les gros fichiers ne sont pas testés.
- Après une erreur de lecture, l'instance mpv est détruite et sa fenêtre se ferme : l'erreur s'affiche dans l'application.
