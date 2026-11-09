# Architecture

## Vue d'ensemble

```
React (src/App.tsx)
  | invoke : connect, list_dir, play, disconnect
  | <-- evenement "playback-error"
  v
lib.rs  (commandes Tauri, AppState)
  |                         |                               |
  v                         v                               v
smb.rs  <-- open_file ---  proxy.rs  <-- HTTP Range --  player.rs (thread libmpv)
  |         read_at        (axum, 127.0.0.1,               |  loadfile <url>
  |                         token)                         |  -> mpv lit le proxy
  v
Samba (SMB)
```

## Modules

| Fichier | Rôle |
|---|---|
| `src/App.tsx` | Interface unique : formulaire de connexion, liste des partages, fil d'Ariane, listing, bandeau d'erreur. Pas de routeur ni de store externe. |
| `src-tauri/src/lib.rs` | Point d'entrée Tauri : `AppState` (connexions, proxy, lecteur), commandes, démarrage du proxy dans `setup`. |
| `src-tauri/src/smb.rs` | Connexions SMB (un `smb::Client` par connexion, dans une `HashMap`), listing filtré, ouverture de fichier. |
| `src-tauri/src/proxy.rs` | Serveur HTTP local (axum) qui sert les fichiers SMB avec gestion de `Range`. |
| `src-tauri/src/player.rs` | Thread unique qui possède l'instance `Mpv` et reçoit les URL par un canal `mpsc`. |
| `src-tauri/build.rs` | Sous Windows : chemin de recherche de `mpv.lib` et copie de `libmpv-2.dll` dans `target/<profil>`. |

## Flux

### Connexion

1. Le formulaire appelle `connect(host, share, username, password)`.
2. `smb::connect` sépare un éventuel `:port` de l'hôte, construit le chemin UNC (`\\hote\partage`), crée un `Client` avec ce port, puis appelle `share_connect`.
3. Un identifiant aléatoire de 64 bits (hexadécimal) est associé au `Client` et à la racine UNC dans la `HashMap`. Le mot de passe n'est pas conservé.
4. La réponse est `{ id, label }`, où `label` vaut `hote/partage` (sans le port).

### Listing

1. `list_dir(connectionId, path)` ouvre le dossier (`create_file`, accès liste) puis lance `Directory::query::<FileFullDirectoryInformation>` avec `*`.
2. Les entrées gardent les sous-dossiers et les fichiers dont l'extension est dans `VIDEO_EXTENSIONS` (`mkv mp4 avi mov webm m4v ts wmv flv`), sans `.` ni `..`.
3. Tri : dossiers d'abord, puis nom en minuscules. Le listing n'est pas récursif. Le chemin `""` désigne la racine et les `/` deviennent des `\`.

### Lecture

1. `play(connectionId, path)` vérifie que la connexion existe, puis calcule l'URL du proxy : `http://127.0.0.1:<port>/<token>/<connexion>/<chemin encodé>`.
2. `player.play(url)` envoie `Command::Play` au thread mpv.
3. Le thread crée l'instance mpv si elle n'existe pas (`force-window`, `osc`, `input-default-bindings`, `input-vo-keyboard`), puis exécute `loadfile <url> replace`.
4. mpv interroge le proxy en HTTP. Chaque requête ouvre son propre handle SMB via `smb::open_file`.
5. Le proxy vérifie le token (sinon `403`). Il lit la taille du fichier (`get_len`) puis traite l'en-tête `Range` :
   - pas de `Range` : `200` avec tout le fichier ;
   - `bytes=a-b`, `bytes=a-` ou suffixe `bytes=-n` : `206` avec `Content-Range` ;
   - intervalle invalide : `416` ;
   - `HEAD` : mêmes en-têtes, sans corps.
6. Le corps est produit par blocs de 1 Mio via `read_at`. Un seek de mpv est une nouvelle requête `Range`.
7. Toute erreur à l'ouverture du fichier (chemin introuvable, connexion inconnue) renvoie `404`. Une erreur pendant le flux termine le corps en erreur.

### Fin de vie des handles SMB

Le handle SMB est déplacé dans le flux du corps. Si mpv coupe la connexion (seek, fin de lecture), hyper abandonne le corps, et le `Drop` de `ResourceHandle` du crate `smb` lance un `close` côté serveur. Vérifié avec `smbstatus` sur le serveur Samba.

## Contrat des commandes

| Commande | Arguments (JS, camelCase) | Retour |
|---|---|---|
| `connect` | `{ host, share, username, password }` ; `host` accepte `hote:port` | `{ id, label }` |
| `list_dir` | `{ connectionId, path }` ; `""` = racine | `[{ name, isDir, size }]` |
| `play` | `{ connectionId, path }` | `()` |
| `disconnect` | `{ connectionId }` | `()` |

Toutes les commandes renvoient `Result<_, String>`, le message d'erreur étant affiché par le bandeau.

Événement `playback-error` : charge utile texte, émise vers le frontend quand mpv ne peut pas être initialisé ou quand la lecture échoue.

## Décisions

### Proxy HTTP plutôt que `smb://` dans mpv

Le support de `smb://` dans mpv dépend du build (libsmbclient). Le proxy fournit une seule implémentation SMB en Rust pur, pour le listing comme pour la lecture, avec les identifiants saisis dans l'application.

### Token aléatoire et bind sur 127.0.0.1

Le proxy écoute sur `127.0.0.1` avec un port choisi par le système (`:0`). Un token aléatoire de 128 bits, généré au lancement, fait partie de chaque URL. Un autre processus local ne peut pas lire les partages sans ce token.

### Thread dédié pour mpv

`libmpv2` et son instance sont confinés dans un thread unique. Les autres parties de l'application ne communiquent avec lui que par le canal `mpsc`. La boucle alterne `try_recv` (commandes) et `wait_event(0.1)` (événements mpv).

Cycle de vie de la fenêtre :
- création paresseuse, au premier `play` ;
- `loadfile replace` pour les lectures suivantes, tant que l'instance existe ;
- destruction de l'instance (donc fermeture de la fenêtre) sur `Shutdown` (au `Drop` de `Player`), sur fin de fichier `Eof` ou `Quit`, ou sur erreur remontée par `wait_event`. Le clic suivant recrée l'instance.

Une fin de fichier de raison `Error` remonte de `wait_event` comme `Some(Err(..))` : elle relève du cas « erreur remontée par `wait_event` » ci-dessus, donc l'instance est détruite et la fenêtre se ferme.

### Choix écartés

- **Client SMB de Windows** (`WNetAddConnection2`) : Windows uniquement. Le crate `smb` est en Rust pur et compile aussi sous Linux.
- **Lecteur intégré à la fenêtre Tauri via `wid`** : complexité supérieure au besoin d'un MVP. La fenêtre mpv reste séparée.
