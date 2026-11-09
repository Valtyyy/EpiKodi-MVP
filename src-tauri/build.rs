use std::{env, fs, path::PathBuf};

fn main() {
    // Le script de build tourne sur l'hôte : on teste la cible à l'exécution.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let libmpv = manifest.join("../libmpv");
        println!("cargo:rustc-link-search=native={}", libmpv.display());
        println!("cargo:rerun-if-changed={}", libmpv.display());

        // OUT_DIR = target/<profil>/build/<crate>-<hash>/out -> remonter à target/<profil>
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        let profile_dir = out.ancestors().nth(3).unwrap();
        fs::copy(libmpv.join("libmpv-2.dll"), profile_dir.join("libmpv-2.dll"))
            .expect("libmpv/libmpv-2.dll introuvable : lancer scripts/setup-libmpv.ps1");
    }
    tauri_build::build()
}
