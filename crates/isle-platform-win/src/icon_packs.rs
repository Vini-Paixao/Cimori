// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
//! Port of DownloadIconPack/ReadIconFolder (mac-island 3.1.1, 33940-34070).
use isle_core::icons::{Icon, IconImages, IconPack};
use std::{collections::BTreeMap, path::{Path, PathBuf}};

pub fn pack_path(root: &Path, pack: IconPack) -> Option<PathBuf> { Some(root.join(pack.source()?.0)) }

// Port of ReadIconFolder/IconGlyphFromName (mac-island 3.1.1): names or Segoe hex codes.
pub fn load_folder(path: &Path, pack: IconPack, folder: String) -> IconImages {
    let mut images = IconImages {pack,folder,..Default::default()};
    let Ok(entries) = std::fs::read_dir(path) else {return images};
    let files: BTreeMap<String,PathBuf> = entries.take(1024).flatten().filter_map(|e| {
        let p = e.path();
        let name = p.file_name()?.to_str()?.to_ascii_lowercase();
        Some((name,p))
    }).collect();
    for icon in Icon::ALL {
        let info = icon.info();
        let stem = pack.source().and_then(|(_,_,_,i)| info.names[i]).unwrap_or(info.name);
        for name in [format!("{stem}.svg"),format!("{stem}.png"),format!("{:04x}.svg",info.glyph),format!("{:04x}.png",info.glyph)] {
            let Some(p) = files.get(&name) else {continue};
            if !std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= 2*1024*1024) {continue;}
            if let Ok(bytes) = std::fs::read(p) { images.pictures.insert(info.glyph,bytes); break; }
        }
    }
    images
}

// Port of DownloadIconPack (mac-island 3.1.1): unique files, 404 markers and atomic replacement.
// Called only for a selected online pack on a runtime worker (never the UI thread).
pub async fn prepare(root: &Path, pack: IconPack, folder: String, progress: impl Fn(usize,usize)) -> anyhow::Result<IconImages> {
    if pack == IconPack::Folder {return Ok(load_folder(Path::new(&folder),pack,folder.clone()));}
    let Some(path) = pack_path(root,pack) else {return Ok(IconImages::default())};
    std::fs::create_dir_all(&path)?;
    let mut names = BTreeMap::new();
    for icon in Icon::ALL {
        if let Some((_,_,_,i)) = pack.source() { if let Some(name) = icon.info().names[i] {
            if !path.join(format!("{name}.svg")).exists() && !path.join(format!("{name}.none")).exists() { names.insert(name,pack.url(icon).unwrap()); }
        } }
    }
    progress(0,names.len());
    if !names.is_empty() {
        // This public CDN needs no Windows credentials; Rustls also works in restricted desktops.
        let client = reqwest::Client::builder().use_rustls_tls().timeout(std::time::Duration::from_secs(5)).build()?;
        for (index,(name,url)) in names.iter().enumerate() {
            let mut response = client.get(url).send().await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND { std::fs::write(path.join(format!("{name}.none")),[])?; }
            else {
                response = response.error_for_status()?;
                let mut data = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    anyhow::ensure!(data.len()+chunk.len() <= 2*1024*1024,"SVG excede o limite"); data.extend_from_slice(&chunk);
                }
                anyhow::ensure!(std::str::from_utf8(&data).is_ok_and(|s|s.contains("<svg")),"Resposta inválida para ícone");
                let part = path.join(format!("{name}.part"));
                std::fs::write(&part,&data)?;
                std::fs::rename(&part,path.join(format!("{name}.svg")))?;
            }
            progress(index+1,names.len());
        }
    }
    Ok(load_folder(&path,pack,folder))
}
