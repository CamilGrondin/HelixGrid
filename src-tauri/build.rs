fn main() {
    #[cfg(target_os = "windows")]
    {
        // Sur Windows, ort avec directml génère / télécharge DirectML.dll
        // S'il est présent dans le répertoire de build, on le copie dans CARGO_MANIFEST_DIR
        // pour que Tauri l'embarque systématiquement dans l'installateur MSI / NSIS.
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let out_path = std::path::PathBuf::from(out_dir);
            if let Some(profile_dir) = out_path.ancestors().nth(3) {
                let dll_src = profile_dir.join("DirectML.dll");
                if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
                    let dll_dest = std::path::Path::new(&manifest_dir).join("DirectML.dll");
                    if dll_src.exists() && !dll_dest.exists() {
                        let _ = std::fs::copy(&dll_src, &dll_dest);
                    }
                }
            }
        }
    }

    tauri_build::build()
}
