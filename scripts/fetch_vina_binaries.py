#!/usr/bin/env python3
"""
HelixGrid — Téléchargement des Binaires Officiels AutoDock Vina Multiplateformes
Télécharge et configure les binaires Vina pour :
- macOS Apple Silicon (aarch64-apple-darwin)
- macOS Intel (x86_64-apple-darwin)
- Windows x64 (x86_64-pc-windows-msvc.exe)
- Linux x64 (x86_64-unknown-linux-gnu)
- Linux ARM (aarch64-unknown-linux-gnu)
"""

import os
import sys
import stat
import urllib.request
import logging
from pathlib import Path

logging.basicConfig(
    level=logging.INFO,
    format="[%(asctime)s] [%(levelname)s] [VinaFetch] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S"
)
logger = logging.getLogger("helixgrid.vina_fetch")

VINA_VERSION = "1.2.7"
BASE_URL = f"https://github.com/ccsb-scripps/AutoDock-Vina/releases/download/v{VINA_VERSION}"

# Mapping des binaires officiels vers les target triples Tauri
BINARIES = {
    # macOS
    "vina_1.2.7_mac_aarch64": "vina-aarch64-apple-darwin",
    "vina_1.2.7_mac_x86_64": "vina-x86_64-apple-darwin",
    # Windows
    "vina_1.2.7_win.exe": "vina-x86_64-pc-windows-msvc.exe",
    # Linux
    "vina_1.2.7_linux_x86_64": "vina-x86_64-unknown-linux-gnu",
    "vina_1.2.7_linux_aarch64": "vina-aarch64-unknown-linux-gnu",
}

ROOT_DIR = Path(__file__).resolve().parent.parent
TAURI_BIN_DIR = ROOT_DIR / "src-tauri" / "bin"


def download_file(url: str, dest_path: Path):
    """Télécharge un fichier avec suivi de progression."""
    dest_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = dest_path.with_suffix(".download.tmp")
    logger.info(f"Téléchargement : {url} -> {dest_path.name}")

    try:
        urllib.request.urlretrieve(url, temp_path)
        temp_path.rename(dest_path)

        # Rendre exécutable sur Unix
        if not dest_path.name.endswith(".exe"):
            current_mode = dest_path.stat().st_mode
            dest_path.chmod(current_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

        logger.info(f"[✓] {dest_path.name} téléchargé avec succès ({dest_path.stat().st_size / (1024*1024):.2f} MB).")
        return True
    except Exception as e:
        logger.error(f"[!] Échec du téléchargement de {url}: {e}")
        if temp_path.exists():
            temp_path.unlink()
        return False


def main():
    logger.info(f"=== Provisionnement des Binaires AutoDock Vina v{VINA_VERSION} ===")
    TAURI_BIN_DIR.mkdir(parents=True, exist_ok=True)

    success_count = 0
    for remote_name, tauri_target_name in BINARIES.items():
        url = f"{BASE_URL}/{remote_name}"
        dest = TAURI_BIN_DIR / tauri_target_name

        if dest.exists() and dest.stat().st_size > 100000:
            logger.info(f"[i] Binaire déjà présent : {tauri_target_name} ({dest.stat().st_size / (1024*1024):.2f} MB)")
            success_count += 1
            continue

        if download_file(url, dest):
            success_count += 1

    # Création des alias standard pour tests locaux
    # Sur Mac ARM : alias 'vina'
    arm_mac = TAURI_BIN_DIR / "vina-aarch64-apple-darwin"
    if arm_mac.exists():
        alias_path = TAURI_BIN_DIR / "vina"
        if not alias_path.exists() or alias_path.stat().st_size != arm_mac.stat().st_size:
            import shutil
            shutil.copy2(arm_mac, alias_path)
            alias_path.chmod(alias_path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            logger.info(f"[✓] Alias local copié : {alias_path}")

    # Sur Windows : alias 'vina.exe'
    win_bin = TAURI_BIN_DIR / "vina-x86_64-pc-windows-msvc.exe"
    if win_bin.exists():
        win_alias = TAURI_BIN_DIR / "vina.exe"
        if not win_alias.exists() or win_alias.stat().st_size != win_bin.stat().st_size:
            import shutil
            shutil.copy2(win_bin, win_alias)
            logger.info(f"[✓] Alias Windows copié : {win_alias}")

    logger.info(f"=== {success_count}/{len(BINARIES)} binaires Vina prêts pour Tauri et le déploiement multiplateforme ===")


if __name__ == "__main__":
    main()
