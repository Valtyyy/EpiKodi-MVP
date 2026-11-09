# AGENTS.md

Consignes pour les agents qui reprennent EpiKodi. Lire `README.md` (utilisation, Samba de test) et `ARCHITECTURE.md` (flux, décisions) avant de modifier le code.

## Commandes

| Action | Commande |
|---|---|
| Installer les dépendances | `npm install` |
| Lancer en développement | `npx tauri dev` |
| Build de débogage sans bundle | `npx tauri build --debug --no-bundle` (appelle `npm run build` via `beforeBuildCommand`) |
| Vérifier le frontend (`tsc` + `vite build`) | `npm run build` |
| Lint Rust | `cargo clippy` dans `src-tauri/` |
| Préparer `libmpv/` (Windows) | `powershell -ExecutionPolicy Bypass -File .\scripts\setup-libmpv.ps1` |

## Structure et où modifier

| Fichier | Rôle | À modifier pour |
|---|---|---|
| `src/App.tsx` | Interface unique | écrans, formulaire, messages |
| `src/App.css` | Styles | apparence |
| `src/main.tsx` | Point d'entrée React | rarement |
| `src-tauri/src/lib.rs` | Commandes Tauri, `AppState`, `setup` | ajouter ou modifier une commande |
| `src-tauri/src/smb.rs` | Connexions, listing, `open_file` | filtre d'extensions (`VIDEO_EXTENSIONS`), options de connexion SMB |
| `src-tauri/src/proxy.rs` | Serveur HTTP local, `Range` | streaming, en-têtes HTTP |
| `src-tauri/src/player.rs` | Thread mpv | options mpv, cycle de vie de la fenêtre |
| `src-tauri/build.rs` | `mpv.lib` et copie de la DLL (Windows) | liens natifs |
| `src-tauri/tauri.conf.json` | Fenêtre, commandes de build, icônes | taille de fenêtre, identifiant |
| `src-tauri/capabilities/default.json` | Permissions Tauri (`core:default`) | ajouter une permission |
| `scripts/setup-libmpv.ps1` | Prépare `libmpv/` | version de libmpv |
| `PROMPT.md` | Brief du projet | ne pas modifier sans demande |

## Contrat des commandes

`lib.rs` et `src/App.tsx` doivent rester synchronisés. Tauri convertit les arguments snake_case de Rust en camelCase côté JavaScript. `Entry` et `Connected` sont sérialisés tels quels.

| Commande | Arguments JS | Retour |
|---|---|---|
| `connect` | `{ host, share, username, password }` | `{ id, label }` (interface `Connection`) |
| `list_dir` | `{ connectionId, path }` | `[{ name, isDir, size }]` (interface `Entry`) |
| `play` | `{ connectionId, path }` | `()` |
| `disconnect` | `{ connectionId }` | `()` |

Événement : `playback-error` (texte), écouté dans `App.tsx`.

## Règles du MVP

- Pas de tests unitaires.
- Pas de sur-ingénierie ni d'abstraction inutile : pas de trait ni de couche sans besoin constaté.
- Pas de commit sans demande explicite.
- Le brief (`PROMPT.md`) prévoit que l'orchestrateur ne code pas : le code est délégué à des sub agents Haiku ou Sonnet.
- Le matériel de test (partages, vidéos, identifiants) reste hors du dépôt.

## Pièges

- `libmpv/` est ignoré par git. Il doit être préparé par `scripts/setup-libmpv.ps1`, sinon `build.rs` échoue.
- `build.rs` ne copie `libmpv-2.dll` que sous Windows. Sous Linux, libmpv vient du paquet système.
- `scripts/setup-libmpv.ps1` appelle `C:\Windows\System32\tar.exe` explicitement : le `tar` de Git Bash ne lit pas les archives 7z.
- Pour un exemple Rust lancé hors de Tauri sous Windows, `target/debug` doit être dans le `PATH` afin de trouver `libmpv-2.dll`.
- `Proxy::start` doit tourner dans le runtime de Tauri (il est appelé via `block_on` dans `setup`, et spawn via `tauri::async_runtime`).
- Sous Windows, le port 445 est occupé : les tests Samba passent par le port 1445.
- Le proxy sert tout fichier du partage aux détenteurs du token, pas seulement les vidéos. Ne pas présenter le filtre d'extensions comme une protection.
- Le listing n'est pas récursif et ne découvre pas les partages d'un serveur.
