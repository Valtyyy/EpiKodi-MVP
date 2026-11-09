import { useEffect, useState, type ChangeEvent, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface Connection {
  id: string;
  label: string;
}

interface Entry {
  name: string;
  isDir: boolean;
  size: number;
}

interface ConnectForm {
  host: string;
  share: string;
  username: string;
  password: string;
}

const EMPTY_FORM: ConnectForm = { host: "", share: "", username: "", password: "" };

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  const units = ["Ko", "Mo", "Go", "To"];
  let value = bytes / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toLocaleString("fr-FR", { maximumFractionDigits: 1 })} ${units[i]}`;
}

function joinPath(dir: string, name: string): string {
  return dir ? `${dir}/${name}` : name;
}

export default function App() {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<ConnectForm>(EMPTY_FORM);
  const [connecting, setConnecting] = useState(false);
  const [path, setPath] = useState("");
  const [entries, setEntries] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  const selected = connections.find((c) => c.id === selectedId) ?? null;
  const segments = path ? path.split("/") : [];

  // Erreurs de lecture asynchrones envoyees par le backend.
  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    listen<string>("playback-error", (event) => setError(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Chargement du dossier courant. Une reponse obsolete (changement de
  // dossier ou de partage entre-temps) est ignoree grace au flag `active`.
  useEffect(() => {
    if (!selectedId) return;
    let active = true;
    setLoading(true);
    setLoadFailed(false);
    invoke<Entry[]>("list_dir", { connectionId: selectedId, path })
      .then((items) => {
        if (active) setEntries(items);
      })
      .catch((err) => {
        if (active) {
          setEntries([]);
          setLoadFailed(true);
          setError(String(err));
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [selectedId, path]);

  function updateField(field: keyof ConnectForm) {
    return (e: ChangeEvent<HTMLInputElement>) =>
      setForm((f) => ({ ...f, [field]: e.target.value }));
  }

  async function connect(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setConnecting(true);
    setError(null);
    try {
      const conn = await invoke<Connection>("connect", {
        host: form.host.trim(),
        share: form.share.trim(),
        username: form.username,
        password: form.password,
      });
      setConnections((prev) => [...prev, conn]);
      setSelectedId(conn.id);
      setPath("");
      setForm((f) => ({ ...f, password: "" }));
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }

  async function disconnect(id: string) {
    try {
      await invoke("disconnect", { connectionId: id });
    } catch (err) {
      setError(String(err));
      return;
    }
    setConnections((prev) => prev.filter((c) => c.id !== id));
    if (selectedId === id) {
      setSelectedId(null);
      setPath("");
      setEntries([]);
    }
  }

  function selectConnection(id: string) {
    setSelectedId(id);
    setPath("");
  }

  async function play(name: string) {
    if (!selectedId) return;
    try {
      await invoke("play", { connectionId: selectedId, path: joinPath(path, name) });
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <h1 className="brand">EpiKodi</h1>

        <form className="connect-form" onSubmit={connect}>
          <fieldset disabled={connecting}>
            <legend>Nouvelle connexion</legend>

            <label htmlFor="host">Hôte</label>
            <input
              id="host"
              name="host"
              placeholder="192.168.1.10 ou hote:445"
              value={form.host}
              onChange={updateField("host")}
              autoComplete="off"
              required
            />

            <label htmlFor="share">Partage</label>
            <input
              id="share"
              name="share"
              placeholder="Videos"
              value={form.share}
              onChange={updateField("share")}
              autoComplete="off"
              required
            />

            <label htmlFor="username">Utilisateur</label>
            <input
              id="username"
              name="username"
              value={form.username}
              onChange={updateField("username")}
              autoComplete="off"
            />

            <label htmlFor="password">Mot de passe</label>
            <input
              id="password"
              name="password"
              type="password"
              value={form.password}
              onChange={updateField("password")}
              autoComplete="current-password"
            />

            <button type="submit" className="primary">
              {connecting ? "Connexion en cours…" : "Connecter"}
            </button>
          </fieldset>
        </form>

        <section className="shares">
          <h2>Partages connectés</h2>
          {connections.length === 0 ? (
            <p className="muted">Aucun partage connecté.</p>
          ) : (
            <ul>
              {connections.map((c) => (
                <li key={c.id} className={c.id === selectedId ? "share active" : "share"}>
                  <button
                    type="button"
                    className="share-select"
                    aria-current={c.id === selectedId ? "true" : undefined}
                    onClick={() => selectConnection(c.id)}
                  >
                    {c.label}
                  </button>
                  <button
                    type="button"
                    className="ghost"
                    aria-label={`Déconnecter ${c.label}`}
                    onClick={() => disconnect(c.id)}
                  >
                    Déconnecter
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </aside>

      <main className="main">
        {error && (
          <div className="error" role="alert">
            <span>{error}</span>
            <button type="button" className="ghost" aria-label="Fermer l'erreur" onClick={() => setError(null)}>
              ×
            </button>
          </div>
        )}

        {!selected ? (
          <p className="empty-state">Aucune connexion. Connectez un partage SMB pour commencer.</p>
        ) : (
          <>
            <nav className="breadcrumb" aria-label="Chemin courant">
              <button type="button" className="crumb" onClick={() => setPath("")}>
                {selected.label}
              </button>
              {segments.map((seg, i) => (
                <span key={i} className="crumb-item">
                  <span className="sep">/</span>
                  <button
                    type="button"
                    className="crumb"
                    onClick={() => setPath(segments.slice(0, i + 1).join("/"))}
                  >
                    {seg}
                  </button>
                </span>
              ))}
            </nav>

            {loading ? (
              <p className="muted">Chargement…</p>
            ) : loadFailed ? null : entries.length === 0 ? (
              <p className="muted">Aucune vidéo dans ce dossier</p>
            ) : (
              <ul className="entries">
                {entries.map((entry) => (
                  <li key={entry.name}>
                    <button
                      type="button"
                      className="entry"
                      onClick={() =>
                        entry.isDir
                          ? setPath(joinPath(path, entry.name))
                          : play(entry.name)
                      }
                    >
                      <span className="kind">{entry.isDir ? "Dossier" : "Vidéo"}</span>
                      <span className="name">{entry.isDir ? `${entry.name}/` : entry.name}</span>
                      <span className="size">{entry.isDir ? "" : formatSize(entry.size)}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </main>
    </div>
  );
}
