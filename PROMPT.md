# Context

Application natif de mediacenter à la Kodi.
Ceci est un MVP.
L'implémentation doit se faire par sub agents Haiku ou Sonnet.
En tant qu'orchestrateur tu ne codes jamais.
Aucun unit tests attendu car ceci est un MVP.
Aucune sur ingénieurie ou abstraction attendu.

# Content

Livré une application très minimale écrit en rust avec tauri comme backend et react comme frontend.
Utiliser libmpv pour lire les videos.
Handle le protocol SMB.

Je dois pouvoir connecter mon app à des backend SMB (samba), que mon app doit afficher ces videos simplement (donc sans miniature), cliquer dessus ouvre la video (avec libmpv).

## Livrable

- Le projet rust
- ARCHITECTURE.md
- AGENTS.md
- README.md
