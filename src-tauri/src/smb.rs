use futures_util::StreamExt;
use serde::Serialize;
use smb::{
    Client, ClientConfig, ConnectionConfig, DirAccessMask, Directory, File, FileAccessMask,
    FileCreateArgs, FileFullDirectoryInformation, Resource, UncPath,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const VIDEO_EXTENSIONS: [&str; 9] = ["mkv", "mp4", "avi", "mov", "webm", "m4v", "ts", "wmv", "flv"];

pub struct Connexion {
    client: Client,
    root: UncPath,
}

pub type Connexions = Arc<Mutex<HashMap<String, Arc<Connexion>>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub async fn connect(
    connexions: &Connexions,
    host: &str,
    share: &str,
    username: &str,
    password: String,
) -> Result<(String, String), String> {
    let (host, port) = match host.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            (h, Some(p.parse::<u16>().map_err(|_| format!("Port invalide : {p}"))?))
        }
        _ => (host, None),
    };
    let root = UncPath::new(host)
        .and_then(|u| u.with_share(share))
        .map_err(|e| format!("Adresse SMB invalide : {e}"))?;
    let client = Client::new(ClientConfig {
        connection: ConnectionConfig {
            port,
            ..Default::default()
        },
        ..Default::default()
    });
    client
        .share_connect(&root, username, password)
        .await
        .map_err(|e| format!("Connexion SMB impossible : {e}"))?;

    let id = format!("{:016x}", rand::random::<u64>());
    connexions
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Connexion { client, root }));
    Ok((id, format!("{host}/{share}")))
}

pub async fn disconnect(connexions: &Connexions, id: &str) -> Result<(), String> {
    let connexion = connexions.lock().unwrap().remove(id);
    if let Some(c) = connexion {
        let _ = c.client.close().await;
    }
    Ok(())
}

fn get(connexions: &Connexions, id: &str) -> Result<Arc<Connexion>, String> {
    connexions
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| "Connexion inconnue".to_string())
}

fn unc(c: &Connexion, path: &str) -> UncPath {
    let path = path.trim_matches('/');
    if path.is_empty() {
        c.root.clone()
    } else {
        c.root.clone().with_path(&path.replace('/', "\\"))
    }
}

pub async fn list_dir(connexions: &Connexions, id: &str, path: &str) -> Result<Vec<Entry>, String> {
    let c = get(connexions, id)?;
    let access: FileAccessMask = DirAccessMask::new()
        .with_list_directory(true)
        .with_synchronize(true)
        .into();
    let resource = c
        .client
        .create_file(&unc(&c, path), &FileCreateArgs::make_open_existing(access))
        .await
        .map_err(|e| format!("Ouverture du dossier impossible : {e}"))?;
    let Resource::Directory(dir) = resource else {
        return Err("Ce chemin n'est pas un dossier".into());
    };
    let dir = Arc::new(dir);
    let items = Directory::query::<FileFullDirectoryInformation>(&dir, "*")
        .await
        .map_err(|e| format!("Listing impossible : {e}"))?
        .collect::<Vec<_>>()
        .await;
    let _ = dir.close().await;

    let mut entries = Vec::new();
    for item in items {
        let item = item.map_err(|e| format!("Listing impossible : {e}"))?;
        let name = item.file_name.to_string();
        let is_dir = item.file_attributes.directory();
        let is_video = name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()));
        if name != "." && name != ".." && (is_dir || is_video) {
            entries.push(Entry {
                name,
                is_dir,
                size: item.end_of_file,
            });
        }
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn has_connexion(connexions: &Connexions, id: &str) -> bool {
    connexions.lock().unwrap().contains_key(id)
}

pub async fn open_file(connexions: &Connexions, id: &str, path: &str) -> Result<File, String> {
    let c = get(connexions, id)?;
    let access = FileAccessMask::new().with_generic_read(true);
    match c
        .client
        .create_file(&unc(&c, path), &FileCreateArgs::make_open_existing(access))
        .await
        .map_err(|e| format!("Ouverture du fichier impossible : {e}"))?
    {
        Resource::File(f) => Ok(f),
        _ => Err("Ce chemin n'est pas un fichier".into()),
    }
}
